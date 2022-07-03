use std::io;
use tokio::sync::mpsc;

type Message = Box<[u8]>;

fn main_input_loop(tx: mpsc::UnboundedSender<Message>) -> io::Result<()> {
    use io::BufRead;

    for maybe_line in io::BufReader::new(io::stdin()).lines() {
        let line = maybe_line?.into_boxed_str();

        // Closing the channel should signal the network thread to join.
        if line.trim() == "/quit" {
            break;
        }

        tx.send(line.into_boxed_bytes()).expect("receiver closed");
    }

    Ok(())
}

fn main() -> io::Result<()> {
    use std::net::Ipv4Addr;
    let addr = std::net::SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0));
    let std_socket = std::net::UdpSocket::bind(addr)?;
    std_socket.set_nonblocking(true)?;
    std_socket.connect((Ipv4Addr::new(224, 0, 0, 69), 3000))?;

    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
    let handle = std::thread::spawn(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .build()?
            .block_on(async move {
                use tokio::io::AsyncWriteExt;
                let mut stdout = tokio::io::stdout();
                let socket = tokio::net::UdpSocket::from_std(std_socket)?;
                let mut buf = [0; 64];

                loop {
                    tokio::select! {
                        count_res = socket.recv(&mut buf) => stdout.write_all(&buf[..count_res?]).await?,
                        input_res = rx.recv() => {
                            if let Some(input) = input_res {
                                socket.send(&input).await?;
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
