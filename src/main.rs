#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;

use app::Message;
use std::io;

fn main() -> io::Result<()> {
    use std::net::{Ipv4Addr, SocketAddrV4};
    const MULTICAST_ADDR: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 69);
    const MULTICAST_PORT: u16 = 3000;
    let bound_addr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, MULTICAST_PORT).into();

    use socket2::{Domain, Protocol, Socket, Type};
    let sock = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    sock.set_reuse_address(true)?;
    sock.set_nonblocking(true)?;
    sock.join_multicast_v4(&MULTICAST_ADDR, &Ipv4Addr::UNSPECIFIED)?;
    sock.bind(&bound_addr)?;

    use tokio::sync::{mpsc, watch};
    let (msg_tx, mut msg_rx) = mpsc::unbounded_channel::<Message>();
    let (log_tx, log_rx) = watch::channel(String::new());
    let handle = std::thread::spawn(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .build()?
            .block_on(async move {
                let socket = tokio::net::UdpSocket::from_std(sock.into())?;
                let mut buf = [0; 64];

                loop {
                    tokio::select! {
                        biased;
                        input_res = msg_rx.recv() => {
                            if let Some(input) = input_res {
                                socket.send_to(&input, (MULTICAST_ADDR, MULTICAST_PORT)).await?;
                            } else {
                                // Sender has closed, therefore we have stopped polling
                                // the standard input. It is time to terminate the program.
                                break;
                            }
                        }
                        recv_res = socket.recv_from(&mut buf) => {
                            let (count, remote_addr) = recv_res?;
                            let message = match core::str::from_utf8(&buf[..count]) {
                                Ok(parsed) => format!("[{remote_addr}]: {parsed}\n"),
                                _ => continue, // Skip invalid messages
                            };
                            log_tx.send_modify(|log| log.push_str(&message));
                        }
                    }
                }

                Ok(())
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
