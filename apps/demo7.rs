use hbb_common::futures::StreamExt;
use hbb_common::tcp::new_listener;
use hbb_common::tokio;
use hbb_common::tokio::net::TcpStream;
use hbb_common::tokio::signal::ctrl_c;
use hbb_common::tokio_util::codec::{Framed, LinesCodec};
use hbb_common::{anyhow::Result, to_socket_addr};

#[tokio::main]
async fn main() -> Result<()> {
    tcp_21117("127.0.0.1:20001").await?;
    println!("{}", 1111);

    Ok(())
}

async fn tcp_21117(addr: &str) -> Result<()> {
    let mut listener_active = new_listener(addr, false).await?;
    loop {
        // Accept the next connection.

        let (stream, addr) = listener_active.accept().await?;

        // Read messages from the client and ignore I/O errors when the client quits.
        read_messages(stream).await?;
    }

    Ok(())
}

async fn read_messages(mut stream: TcpStream) -> Result<()> {
    let mut lines = Framed::new(stream, LinesCodec::new());
    let username = match lines.next().await {
        Some(Ok(line)) => {
            println!("{:?}", line);
            line
        }

        // We didn't get a line so we return early here.
        _ => {
            println!("{}", 2222);
            String::from("hah")
        }
    };

    Ok(())
}
