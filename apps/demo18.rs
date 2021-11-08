
use hbb_common::{tokio, to_socket_addr, ResultType};
use hbb_common::tokio::net::UdpSocket;

use hbb_common::tokio_util::codec::{BytesCodec, Framed};
use hbb_common::futures::{StreamExt, SinkExt};
use hbb_common::bytes::Bytes;
use hbb_common::tokio_util::udp::UdpFramed;

#[tokio::main]
async fn main()->ResultType<()> {
    let mut socket1 = UdpSocket::bind(to_socket_addr("0.0.0.0:7070").unwrap()).await?;
    let mut socket = UdpFramed::new(socket1, BytesCodec::new());

    loop {
        if let Some(Ok((bytes, addr))) = socket.next().await {
            println!("xxxxxxxxxxx{}",addr );
            socket
                .send((Bytes::from("aaaaaaaa"), addr.clone()))
                .await;

        }
    }
    Ok(())
}