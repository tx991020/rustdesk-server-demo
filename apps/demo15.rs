use hbb_common::{tcp, tokio, ResultType};
use hbb_common::tokio::io::{AsyncWriteExt, AsyncReadExt};
use hbb_common::bytes;




//代理3389 在xp上编译个socket
#[tokio::main]
async fn main()->ResultType<()> {
    let listener = tcp::new_listener(format!("0.0.0.0:{}",6000), true).await?;

    loop {
        // Asynchronously wait for an inbound socket.
        let (mut socket, _) = listener.accept().await?;

        // And this is where much of the magic of this server happens. We
        // crucially want all clients to make progress concurrently, rather than
        // blocking one on completion of another. To achieve this we use the
        // `tokio::spawn` function to execute the work in the background.
        //
        // Essentially here we're executing a new task to run concurrently,
        // which will allow all of our clients to be processed concurrently.

        tokio::spawn(async move {
            let mut buf = vec![0; 1024];

            // In a loop, read data from the socket and write the data back.
            loop {
                let n = socket
                    .read(&mut buf)
                    .await
                    .expect("failed to read data from socket");

                if n == 0 {
                    return;
                }
                let bytes1 = bytes::Bytes::from("wowow");
                socket
                    .write_all(bytes1.as_ref())
                    .await
                    .expect("failed to write data to socket");
            }
        });
    }
    Ok(())
}

