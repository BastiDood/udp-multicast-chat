#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;

fn main() -> std::io::Result<()> {
    use std::net::{Ipv4Addr, SocketAddrV4};
    const MULTICAST_ADDR: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 69);
    const MULTICAST_PORT: u16 = 3000;
    let bound_addr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, MULTICAST_PORT).into();

    use socket2::{Domain, Protocol, Socket, Type};
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_reuse_address(true)?;
    socket.set_nonblocking(true)?;
    socket.join_multicast_v4(&MULTICAST_ADDR, &Ipv4Addr::UNSPECIFIED)?;
    socket.bind(&bound_addr)?;

    let runtime = tokio::runtime::Builder::new_current_thread().enable_io().build()?;
    let udp = {
        let _guard = runtime.enter();
        tokio::net::UdpSocket::from_std(socket.into())?
    };

    use tokio::sync::{mpsc, watch};
    let (msg_tx, mut msg_rx) = mpsc::unbounded_channel::<app::Message>();
    let (log_tx, log_rx) = watch::channel(String::new());
    let handle = std::thread::spawn(move || {
        runtime.block_on(async move {
            let mut buf = [0; 64];
            loop {
                tokio::select! {
                    biased;
                    input_res = msg_rx.recv() => {
                        if let Some(input) = input_res {
                            udp.send_to(&input, (MULTICAST_ADDR, MULTICAST_PORT)).await.expect("cannot send message to socket");
                        } else {
                            // Sender has closed, therefore we have stopped polling
                            // the standard input. It is time to terminate the program.
                            break;
                        }
                    }
                    recv_res = udp.recv_from(&mut buf) => {
                        let (count, remote_addr) = recv_res.expect("cannot receive from socket");
                        if let Ok(parsed) = core::str::from_utf8(&buf[..count]) {
                            log_tx.send_modify(|log| {
                                use core::fmt::Write;
                                log.write_fmt(format_args!("[{remote_addr}]: {parsed}\n")).expect("cannot append message to buffer");
                            });
                        }

                    }
                }
            }
        })
    });

    eframe::run_native(
        "Chat Room",
        Default::default(),
        Box::new(|eframe::CreationContext { egui_ctx, .. }| {
            egui_ctx.set_visuals(eframe::egui::Visuals::dark());
            Box::new(app::App::new(handle, msg_tx, log_rx))
        }),
    )
}
