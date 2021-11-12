use hbb_common::bytes::Bytes;
use hbb_common::tokio::io::{AsyncReadExt, AsyncWriteExt};
use hbb_common::tokio::net::TcpListener;
use hbb_common::{tokio, ResultType};

#[tokio::main]
async fn main() -> ResultType<()> {
    let listener = TcpListener::bind("127.0.0.1:13389").await?;

    loop {
        // Asynchronously wait for an inbound socket.
        let (mut socket, _) = listener.accept().await?;

        tokio::spawn(async move {
            let mut buf = c;

            // In a loop, read data from the socket and write the data back.
            loop {
                let n = socket
                    .read(&mut buf)
                    .await
                    .expect("failed to read data from socket");

                if n == 0 {
                    return;
                }

                println!("111,{:?}", &buf[0..n]);
                socket
                    .write_all(&buf[0..n])
                    .await
                    .expect("failed to write data to socket");
            }
        });
    }
}
