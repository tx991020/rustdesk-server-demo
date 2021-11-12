use hbb_common::tokio::select;
use hbb_common::udp::FramedSocket;
use hbb_common::{sleep, to_socket_addr, tokio, ResultType};
use std::collections::HashSet;
use std::io;
use std::time::Duration;

use hbb_common::anyhow::Context;
use hbb_common::bytes::Bytes;
use hbb_common::config::RENDEZVOUS_TIMEOUT;
use hbb_common::tcp::FramedStream;

use hbb_common::futures::{SinkExt, StreamExt};
use hbb_common::protobuf::Message;
use hbb_common::rendezvous_proto::{RegisterPeer, RendezvousMessage};
use hbb_common::tokio::io::AsyncRead;
use hbb_common::tokio::io::AsyncWriteExt;
use hbb_common::tokio::net::{TcpSocket, TcpStream, UdpSocket};
use hbb_common::tokio_util::codec::{BytesCodec, Framed, LengthDelimitedCodec};
use hbb_common::tokio_util::udp::UdpFramed;

//第一步维护注册表，记录在内存中
//维护配对关系
//第三步只接收对方的消息
//udp_chat_demo
#[tokio::main]
async fn main() -> ResultType<()> {
    let socket1 = UdpSocket::bind("0.0.0.0:0").await?;
    // let mut socket1 = UdpSocket::connect("0.0.0.0:0", ()).await?;
    // let mut stream = TcpStream::connect("127.0.0.1:21116").await?;

    // println!("{:?}",socket.get_ref().local_addr() );
    let mut socket = UdpFramed::new(socket1, LengthDelimitedCodec::new());
    let mut msg = RendezvousMessage::new();
    msg.set_register_peer(RegisterPeer {
        id: "12344".to_string(),
        serial: 0,
        ..Default::default()
    });

    // socket.send(Bytes::from("haha")).await;
    socket
        .send((
            Bytes::from(msg.write_to_bytes().unwrap()),
            to_socket_addr("127.0.0.1:21116").unwrap(),
        ))
        .await;

    loop {
        select! {
            Some(Ok(bytes)) = socket.next() => {
                println!("addr   {:?}",bytes);

            }
            else => break,
        }
    }
    // let mut stream = FramedStream::new(
    //     to_socket_addr("127.0.0.1:21110").unwrap(),
    //     "0.0.0.0:0",
    //     RENDEZVOUS_TIMEOUT,
    // )
    //     .await
    //     .with_context(|| "Failed to connect to rendezvous server")?;
    //
    //
    //     stream.send_bytes(Bytes::from("qwertyhhh")).await;

    Ok(())
}
