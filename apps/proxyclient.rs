use hbb_common::futures::FutureExt;
use hbb_common::tokio::io;
use hbb_common::tokio::net::TcpStream;
use hbb_common::{tokio, ResultType};

use hbb_common::tokio::io::AsyncWriteExt;

#[tokio::main]
async fn main() -> ResultType<()> {
    let mut stream = TcpStream::connect("39.107.33.253:13389").await?;

    let forward = TcpStream::connect("127.0.0.1:3389").await?;

    transfer(stream, forward).await;
    Ok(())
}

async fn transfer(mut inbound: TcpStream, mut outbound: TcpStream) -> ResultType<()> {
    let (mut ri, mut wi) = inbound.split();
    let (mut ro, mut wo) = outbound.split();

    let client_to_server = async {
        io::copy(&mut ri, &mut wo).await?;
        wo.shutdown().await
    };

    let server_to_client = async {
        io::copy(&mut ro, &mut wi).await?;
        wi.shutdown().await
    };

    tokio::try_join!(client_to_server, server_to_client)?;

    Ok(())
}
