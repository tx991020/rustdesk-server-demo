use hbb_common::tcp::{new_listener, FramedStream};
use hbb_common::tokio;
use hbb_common::udp::FramedSocket;
use hbb_common::{anyhow::Result, to_socket_addr};
use std::collections::HashSet;

use hbb_common::message_proto::{message, Message};
use hbb_common::protobuf::Message as _;
use hbb_common::tokio::net::TcpStream;
use hbb_common::tokio::signal::ctrl_c;
#[macro_use]
extern crate tracing;

#[tokio::main]
async fn main() -> Result<()> {
    // tcp_21117("127.0.0.1:20000").await?;
    tokio::spawn(tcp_21117("127.0.0.1:20000"));
    println!("{}", 1111);
    ctrl_c().await?;
    Ok(())
}

async fn tcp_21117(addr: &str) -> Result<()> {
    let mut listener_active = new_listener(addr, false).await?;

    loop {
        // Accept the next connection.

        let (stream, addr) = listener_active.accept().await?;
        // Read messages from the client and ignore I/O errors when the client quits.
        println!("{:?}", addr);
        read_messages(stream).await?;
    }

    Ok(())
}

async fn read_messages(mut stream: TcpStream) -> Result<()> {
    let mut stream = FramedStream::from(stream);

    if let Some(Ok(bytes)) = stream.next_timeout(3000).await {
        println!("1111{:?}", &bytes);
        if let Ok(msg_in) = Message::parse_from_bytes(&bytes) {
            match msg_in.union {
                Some(message::Union::signed_id(hash)) => {
                    info!("{:?}", &hash);
                }
                Some(message::Union::public_key(hash)) => {
                    info!("{:?}", &hash);
                }
                Some(message::Union::test_delay(hash)) => {
                    info!("{:?}", &hash);
                }
                Some(message::Union::video_frame(hash)) => {
                    info!("{:?}", &hash);
                }
                Some(message::Union::login_request(hash)) => {
                    info!("{:?}", &hash);
                }
                Some(message::Union::login_response(hash)) => {
                    info!("{:?}", &hash);
                }
                Some(message::Union::hash(hash)) => {
                    info!("{:?}", &hash);
                }
                Some(message::Union::mouse_event(hash)) => {
                    info!("{:?}", &hash);
                }
                Some(message::Union::audio_frame(hash)) => {
                    info!("{:?}", &hash);
                }
                Some(message::Union::cursor_data(hash)) => {
                    info!("{:?}", &hash);
                }
                Some(message::Union::cursor_position(hash)) => {
                    info!("{:?}", &hash);
                }
                Some(message::Union::cursor_id(hash)) => {
                    info!("{:?}", &hash);
                }
                Some(message::Union::cursor_position(hash)) => {
                    info!("{:?}", &hash);
                }
                Some(message::Union::cursor_id(hash)) => {
                    info!("{:?}", &hash);
                }
                Some(message::Union::key_event(hash)) => {
                    info!("{:?}", &hash);
                }
                Some(message::Union::clipboard(hash)) => {
                    info!("{:?}", &hash);
                }
                Some(message::Union::file_action(hash)) => {
                    info!("{:?}", &hash);
                }
                Some(message::Union::file_response(hash)) => {
                    info!("{:?}", &hash);
                }
                Some(message::Union::misc(hash)) => {
                    info!("{:?}", &hash);
                }
                _ => {
                    println!("99999 {:?}", &bytes);
                }
            }
        } else {
            info!("not match {:?}", &bytes);
        }
    }
    drop(stream);
    Ok(())
}
