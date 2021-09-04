use crate::tokio::signal::ctrl_c;
use crate::tokio::time::interval;
use hbb_common::message_proto::{message, Message};
use hbb_common::tokio::sync::mpsc;
use hbb_common::{
    anyhow::Result,
    bytes::BytesMut,
    message_proto::*,
    protobuf,
    protobuf::Message as _,
    rendezvous_proto::*,
    sleep,
    tcp::{new_listener, FramedStream},
    to_socket_addr, tokio,
    udp::FramedSocket,
    utils, AddrMangle,
};
use std::net::SocketAddr;
use std::ops::Deref;
use std::time::Duration;
use uuid::Uuid;

#[macro_use]
extern crate tracing;

use async_channel::{bounded, Receiver, Sender};
use hbb_common::message_proto::message::Union;
use hbb_common::sodiumoxide::crypto::stream::stream;
use hbb_common::tokio::net::TcpStream;
use hbb_common::tokio::task::JoinHandle;
use std::collections::HashMap;
use std::io::Error;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing_subscriber;

#[derive(Debug, Clone)]
struct client {
    //心跳
    timestamp: u64,
    local_addr: std::net::SocketAddr,
    peer_addr: Option<std::net::SocketAddr>,
}

enum Event {
    /// A client has joined.
    Join(SocketAddr, Arc<TcpStream>),

    /// A client has left.
    Leave(SocketAddr),

    /// A client sent a message.
    Message(SocketAddr, String),
}

//默认情况下，hbbs 侦听 21115(tcp) 和 21116(tcp/udp)，hbbr 侦听 21117(tcp)。请务必在防火墙中打开这些端口
//上线离线问题, id,ip

pub fn get_time() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0) as _
}

#[tokio::main(basic_scheduler)]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    // let mut listener_passive = new_listener("0.0.0.0:21117", false).await.unwrap();
    // let mut listener_passive1 = new_listener("0.0.0.0:21118", false).await.unwrap();

    //两个quic raw_data channel
    let (tx_from_active, mut rx_from_active) = async_channel::unbounded::<Vec<u8>>();
    let (tx_from_passive, mut rx_from_passive) = async_channel::unbounded::<Vec<u8>>();

    // tcp_demo("0.0.0.0:21116").await?;
    let mut id_map: Arc<Mutex<HashMap<String, client>>> = Arc::new(Mutex::new(HashMap::new()));
    // udp_demo("", &mut id_map);
    // tcp_21117("0.0.0.0:21117").await?;
    tokio::spawn(udp_21116("47.88.2.164:21116", id_map));
    tokio::spawn(tcp_21116("47.88.2.164:21117"));
    tokio::spawn(tcp_21117("47.88.2.164:21118"));
    tokio::spawn(tcp_21118("47.88.2.164:21119"));
    tokio::spawn(tcp_21119("47.88.2.164:21116"));

    ctrl_c().await?;
    Ok(())
}

async fn tcp_21116(addr: &str) -> Result<()> {
    let mut listener_active = new_listener(addr, false).await?;
    loop {
        // Accept the next connection.

        let (stream, addr) = listener_active.accept().await?;

        // Read messages from the client and ignore I/O errors when the client quits.
        read_messages(stream).await?;
    }

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

async fn tcp_21118(addr: &str) -> Result<()> {
    let mut listener_active = new_listener(addr, false).await?;
    loop {
        let (stream, addr) = listener_active.accept().await?;

        // Read messages from the client and ignore I/O errors when the client quits.
        read_messages(stream).await?;
    }

    Ok(())
}

async fn tcp_21119(addr: &str) -> Result<()> {
    let mut listener_active = new_listener(addr, false).await?;
    loop {
        let (stream, addr) = listener_active.accept().await?;
        // Accept the next connection.
        let (stream, addr) = listener_active.accept().await?;

        // Read messages from the client and ignore I/O errors when the client quits.
        read_messages(stream).await?;
    }

    Ok(())
}

async fn tcp_read_rendezvous_message(
    mut stream: TcpStream,
    id_map: Arc<Mutex<HashMap<String, client>>>,
) -> Result<()> {
    let mut stream = FramedStream::from(stream);

    if let Some(Ok(bytes)) = stream.next_timeout(3000).await {
        if let Ok(msg_in) = RendezvousMessage::parse_from_bytes(&bytes) {
            match msg_in.union {
                Some(rendezvous_message::Union::local_addr(ph)) => {
                    info!("{:?}", &ph);
                    let remote_desk_id = ph.id;
                    let mut id_map = id_map.lock().await;
                    if let Some(client) = id_map.get(&remote_desk_id) {
                        udp_send_request_relay(
                            &mut socket,
                            "47.88.2.164:21117".to_string(),
                            client.local_addr,
                        )
                            .await;
                    }
                }
                Some(rendezvous_message::Union::relay_response(ph)) => {
                    info!("{:?}", &ph);
                    let mut msg_out = RendezvousMessage::new();
                    msg_out.set_relay_response(RelayResponse {
                        relay_server: "47.88.2.164:21117".to_string(),
                        ..Default::default()
                    });
                    stream.send(&msg_out).await;
                }
                Some(rendezvous_message::Union::test_nat_request(ph)) => {
                    let mut msg_out = RendezvousMessage::new();
                    msg_out.set_test_nat_response(TestNatResponse {
                        port: 0,
                        cu: protobuf::MessageField::some(ConfigUpdate {
                            ..Default::default()
                        }),
                        ..Default::default()
                    });
                    stream.send(&msg_out).await;
                }
                Some(message::Union::video_frame(hash)) => {
                    info!("{:?}", &hash);
                }
                Some(message::Union::login_request(hash)) => {
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
    Ok(())
}

async fn udp_21116(host: &str, id_map: Arc<Mutex<HashMap<String, client>>>) -> Result<()> {
    let mut socket = FramedSocket::new(to_socket_addr(host).unwrap()).await?;
    loop {
        if let Some(Ok((bytes, addr))) = socket.next().await {
            handle_udp(&mut socket, bytes, addr, id_map.clone()).await;
        }
    }
    Ok(())
}

async fn udp_send_register_peer_response(
    socket: &mut FramedSocket,
    bytes: BytesMut,
    addr: std::net::SocketAddr,
    id_map: &mut std::collections::HashMap<String, std::net::SocketAddr>,
) {
    // let mut msg_out = FetchLocalAddr::new();
    // msg_out.set_fetch_local_addr(msg_out);
    // socket.send(&msg_out, addr).await.ok();
}

//被控第一步//从中间收

async fn udp_send_fetch_local_addr(
    socket: &mut FramedSocket,
    relay_server: String,
    addr: std::net::SocketAddr,
) -> Result<()> {
    let mut msg_out = RendezvousMessage::new();
    let addr1 = socket
        .get_ref()
        .local_addr()
        .unwrap_or(to_socket_addr("47.88.2.164:21117").unwrap());
    let vec1 = AddrMangle::encode(addr1);
    msg_out.set_fetch_local_addr(FetchLocalAddr {
        socket_addr: vec1,
        relay_server,
        ..Default::default()
    });
    socket.send(&msg_out, addr).await?;
    Ok(())
}

async fn udp_send_punch_hole(
    socket: &mut FramedSocket,
    bytes: BytesMut,
    addr: std::net::SocketAddr,
    id_map: &mut std::collections::HashMap<String, std::net::SocketAddr>,
) {
    // let mut msg_out = FetchLocalAddr::new();
    // msg_out.set_fetch_local_addr(msg_out);
    // socket.send(&msg_out, addr).await.ok();
}

async fn udp_send_configure_update(
    socket: &mut FramedSocket,
    bytes: BytesMut,
    addr: std::net::SocketAddr,
    id_map: &mut std::collections::HashMap<String, std::net::SocketAddr>,
) {
    // let mut msg_out = FetchLocalAddr::new();
    // msg_out.set_fetch_local_addr(msg_out);
    // socket.send(&msg_out, addr).await.ok();
}

async fn udp_send_request_relay(
    socket: &mut FramedSocket,
    relay_server: String,
    addr: std::net::SocketAddr,
) -> Result<()> {
    let mut msg_out = RendezvousMessage::new();

    msg_out.set_request_relay(RequestRelay {
        relay_server,
        ..Default::default()
    });
    //通过udp回复客户端打洞请求,提供一个中继cdn地址
    socket.send(&msg_out, addr).await?;
    Ok(())
}

async fn handle_udp(
    socket: &mut FramedSocket,
    bytes: BytesMut,
    addr: std::net::SocketAddr,
    id_map: Arc<Mutex<HashMap<String, client>>>,
) {
    if let Ok(msg_in) = RendezvousMessage::parse_from_bytes(&bytes) {
        match msg_in.union {
            //不停的register_peer保持心跳,检测心跳告诉对方不在线
            Some(rendezvous_message::Union::register_peer(rp)) => {
                info!("register_peer {:?}", &addr);
                let mut msg_out = RendezvousMessage::new();
                let mut id_map = id_map.lock().await;
                if let Some(client) = id_map.get(&rp.id) {
                    msg_out.set_register_peer_response(RegisterPeerResponse {
                        request_pk: false,
                        ..Default::default()
                    });
                } else {
                    msg_out.set_register_peer_response(RegisterPeerResponse {
                        request_pk: true,
                        ..Default::default()
                    });
                };
                id_map.insert(
                    rp.id,
                    client {
                        timestamp: utils::now(),
                        local_addr: addr,
                        peer_addr: None,
                    },
                );
                socket.send(&msg_out, addr.clone()).await.ok();
            }
            //完成
            Some(rendezvous_message::Union::register_pk(rp)) => {
                info!("register_pk{}", 22222);
                let mut msg_out = RendezvousMessage::new();
                msg_out.set_register_pk_response(RegisterPkResponse {
                    result: register_pk_response::Result::OK.into(),
                    ..Default::default()
                });
                socket.send(&msg_out, addr).await.ok();
            }
            //暂存没用
            Some(rendezvous_message::Union::configure_update(rp)) => {
                println!("register_peer {:?}", addr);
                // id_map.insert(rp.id, addr);
                // let mut msg_out = ConfigUpdate::new();
                // msg_out.set_configure_update(msg_out);
                // socket.send(&msg_out, addr).await.ok();
            }
            //暂时没用
            Some(rendezvous_message::Union::software_update(rp)) => {
                println!("register_peer {:?}", addr);
                // id_map.insert(rp.id, addr);
                // let mut msg_out = SoftwareUpdate::new();
                // msg_out.set_software_update(msg_out);
                // socket.send(&msg_out, addr).await.ok();
            }

            _ => {
                println!("不匹配的指令");
            }
        }
    }
}
