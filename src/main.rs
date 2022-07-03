use std::io;
use tokio::sync::mpsc;

type Message = Box<[u8]>;

fn main_input_loop(tx: mpsc::UnboundedSender<Message>) -> io::Result<()> {
    use io::BufRead;
    let mut reader = io::BufReader::new(io::stdin());
    let mut buf = String::with_capacity(32);

    loop {
        reader.read_line(&mut buf)?;
        if buf.trim_end() == "/quit" {
            // Closing the channel should signal the network thread to join.
            break;
        }

        let msg = core::mem::take(&mut buf).into_bytes();
        tx.send(msg.into_boxed_slice()).expect("receiver closed");
    }

    Ok(())
}

fn main() -> io::Result<()> {
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

    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
    let handle = std::thread::spawn(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .build()?
            .block_on(async move {
                use tokio::io::AsyncWriteExt;
                let mut stdout = tokio::io::stdout();
                let socket = tokio::net::UdpSocket::from_std(socket.into())?;
                let mut buf = [0; 64];

                loop {
                    tokio::select! {
                        count_res = socket.recv(&mut buf) => stdout.write_all(&buf[..count_res?]).await?,
                        input_res = rx.recv() => {
                            if let Some(input) = input_res {
                                socket.send_to(&input, (MULTICAST_ADDR, MULTICAST_PORT)).await?;
                            } else {
                                // Sender has closed, therefore we have stopped polling
                                // the standard input. It is time to terminate the program.
                                break;
                            }
                        }
                    }
                }

                Ok(())
            })
    });

    let input_result = main_input_loop(tx);
    handle
        .join()
        .expect("cannot join network thread")
        .and(input_result)
}
