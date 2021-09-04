use crate::tokio::select;
use crate::tokio::signal::ctrl_c;
use crate::tokio::time::interval;
use hbb_common::message_proto::{message, Message};
use hbb_common::tokio::sync::{mpsc, RwLock};
use hbb_common::{
    anyhow::Result,
    anyhow::anyhow,
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
use std::ops::{Deref, DerefMut};
use std::time::Duration;
use uuid::Uuid;

#[macro_use]
extern crate tracing;

use crate::tokio::sync::Mutex;
use async_channel::{bounded, Receiver, Sender};
use hbb_common::message_proto::message::Union;
use hbb_common::sodiumoxide::crypto::stream::stream;
use hbb_common::tokio::net::{TcpStream, UdpSocket};
use hbb_common::tokio::task::JoinHandle;
use std::borrow::{Borrow, BorrowMut};
use std::collections::HashMap;
use std::io::Error;
use std::sync::Arc;
use tracing_subscriber;
use std::cell::RefCell;
use futures::future::BoxFuture;
use futures::FutureExt;

#[derive(Debug, Clone)]
struct client {
    //心跳
    timestamp: u64,
    local_addr: std::net::SocketAddr,

}

#[derive(Debug, Clone)]
enum Event {
    /// 打洞第一步
    First(SocketAddr),

    ///打洞第二步
    Second(SocketAddr),
    UnNone,
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

    let (ip_sender, mut ip_rcv) = async_channel::unbounded::<Event>();

    // tcp_demo("0.0.0.0:21116").await?;
    let mut id_map: Arc<Mutex<HashMap<String, client>>> = Arc::new(Mutex::new(HashMap::new()));
    // udp_demo("", &mut id_map);
    // tcp_21117("0.0.0.0:21117").await?;
    let mut socket = FramedSocket::new(to_socket_addr("0.0.0.0:21116").unwrap()).await?;


    tokio::spawn(udp_21116(socket, id_map.clone(), ip_rcv.clone()));

    tokio::spawn(tcp_passive_21117("0.0.0.0:21117", id_map.clone(), ip_sender.clone()));
    tokio::spawn(tcp_active_21116("0.0.0.0:21116", id_map, ip_sender.clone()));
    tokio::spawn(tcp_21118("0.0.0.0:21118"));
    tokio::spawn(tcp_21119("0.0.0.0:21119"));

    ctrl_c().await?;
    Ok(())
}

async fn tcp_active_21116(
    addr: &str,
    id_map: Arc<Mutex<HashMap<String, client>>>,
    sender: Sender<Event>,
) -> Result<()> {
    let mut listener_active = new_listener(addr, false).await?;
    loop {
        // Accept the next connection.

        let (stream, addr) = listener_active.accept().await?;

        // Read messages from the client and ignore I/O errors when the client quits.
        tcp_21116_read_rendezvous_message(stream, id_map.clone(), sender.clone()).await?;
    }

    Ok(())
}

async fn tcp_passive_21117(addr: &str,
   id_map: Arc<Mutex<HashMap<String, client>>>,
   sender: Sender<Event>, ) -> Result<()> {
    let mut listener_active = new_listener(addr, false).await?;
    loop {
        // Accept the next connection.

        let (stream, addr) = listener_active.accept().await?;

        // Read messages from the client and ignore I/O errors when the client quits.
        tcp_21117_read_rendezvous_message(stream,id_map.clone(),sender.clone()).await;
    }

    Ok(())
}

async fn tcp_21117_read_rendezvous_message(
    mut stream: TcpStream,
    id_map: Arc<Mutex<HashMap<String, client>>>,
    sender: Sender<Event>,
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
                        sender.send(Event::Second(client.local_addr)).await;
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
                Some(rendezvous_message::Union::punch_hole_request(ph)) => {
                    info!("{:?}", &ph);
                    let addr1 = stream.get_ref().local_addr()?;
                    let mut msg_out = RendezvousMessage::new();
                    msg_out.set_punch_hole_response(PunchHoleResponse {
                        socket_addr: AddrMangle::encode(addr1),
                        pk: vec![] as Vec<u8>,
                        relay_server: "47.88.2.164:21116".to_string(),
                        union: std::option::Option::Some(punch_hole_response::Union::is_local(
                            false,
                        )),
                        ..Default::default()
                    });
                    if stream.send(&msg_out).await.is_ok() {
                        let remote_desk_id = ph.id;
                        let mut id_map = id_map.lock().await;
                        if let Some(client) = id_map.get(&remote_desk_id) {
                            sender.send(Event::First(client.local_addr)).await;
                        }
                    }
                }
                Some(rendezvous_message::Union::request_relay(ph)) => {
                    let mut msg_out = RendezvousMessage::new();
                    msg_out.set_relay_response(RelayResponse {
                        socket_addr: vec![],
                        uuid: "".to_string(),
                        relay_server: "".to_string(),
                        refuse_reason: "".to_string(),
                        union: None,
                        unknown_fields: Default::default(),
                        cached_size: Default::default(),
                    });
                    stream.send(&msg_out).await;
                }

                _ => {
                    println!("tcp_read_rendezvous_message {:?}", &msg_in);
                }
            }
        } else {
            info!("not match {:?}", &bytes);
        }
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

async fn tcp_21116_read_rendezvous_message(
    mut stream: TcpStream,
    id_map: Arc<Mutex<HashMap<String, client>>>,
    sender: Sender<Event>,
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
                        sender.send(Event::Second(client.local_addr)).await;
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
                Some(rendezvous_message::Union::punch_hole_request(ph)) => {
                    info!("{:?}", &ph);
                    let addr1 = stream.get_ref().local_addr()?;
                    let mut msg_out = RendezvousMessage::new();
                    msg_out.set_punch_hole_response(PunchHoleResponse {
                        socket_addr: AddrMangle::encode(addr1),
                        pk: vec![] as Vec<u8>,
                        relay_server: "47.88.2.164:21116".to_string(),
                        union: std::option::Option::Some(punch_hole_response::Union::is_local(
                            false,
                        )),
                        ..Default::default()
                    });
                    if stream.send(&msg_out).await.is_ok() {
                        let remote_desk_id = ph.id;
                        let mut id_map = id_map.lock().await;
                        if let Some(client) = id_map.get(&remote_desk_id) {
                            sender.send(Event::First(client.local_addr)).await;
                        }
                    }
                }
                Some(rendezvous_message::Union::request_relay(ph)) => {
                    let mut msg_out = RendezvousMessage::new();
                    msg_out.set_relay_response(RelayResponse {
                        socket_addr: vec![],
                        uuid: "".to_string(),
                        relay_server: "".to_string(),
                        refuse_reason: "".to_string(),
                        union: None,
                        unknown_fields: Default::default(),
                        cached_size: Default::default(),
                    });
                    stream.send(&msg_out).await;
                }

                _ => {
                    println!("tcp_read_rendezvous_message {:?}", &msg_in);
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
    let addr = stream.get_ref().local_addr()?;
    if let Some(Ok(bytes)) = stream.next_timeout(3000).await {
        if let Ok(msg_in) = Message::parse_from_bytes(&bytes) {
            match msg_in.union {
                Some(message::Union::signed_id(hash)) => {
                    info!("signed_id {:?}", &hash);
                }
                Some(message::Union::public_key(hash)) => {
                    info!("public_key {:?}", &hash);
                }
                Some(message::Union::test_delay(hash)) => {
                    info!("test_delay {:?}", &hash);
                }
                Some(message::Union::video_frame(hash)) => {
                    info!("video_frame {:?}", &hash);
                }
                Some(message::Union::login_request(hash)) => {
                    info!("login_request {:?}", &hash);
                }
                Some(message::Union::login_response(hash)) => {
                    info!("login_response {:?}", &hash);
                }
                Some(message::Union::hash(hash)) => {
                    info!("hash {:?}", &hash);
                }
                Some(message::Union::mouse_event(hash)) => {
                    info!("mouse_event {:?}", &hash);
                }
                Some(message::Union::audio_frame(hash)) => {
                    info!("audio_frame {:?}", &hash);
                }
                Some(message::Union::cursor_data(hash)) => {
                    info!("cursor_data {:?}", &hash);
                }
                Some(message::Union::cursor_position(hash)) => {
                    info!("cursor_position {:?}", &hash);
                }
                Some(message::Union::cursor_id(hash)) => {
                    info!("cursor_id{:?}", &hash);
                }
                Some(message::Union::cursor_position(hash)) => {
                    info!("cursor_position {:?}", &hash);
                }
                Some(message::Union::cursor_id(hash)) => {
                    info!("cursor_id {:?}", &hash);
                }
                Some(message::Union::key_event(hash)) => {
                    info!("key_event {:?}", &hash);
                }
                Some(message::Union::clipboard(hash)) => {
                    info!("clipboard {:?}", &hash);
                }
                Some(message::Union::file_action(hash)) => {
                    info!("file_action {:?}", &hash);
                }
                Some(message::Union::file_response(hash)) => {
                    info!("file_response {:?}", &hash);
                }
                Some(message::Union::misc(hash)) => {
                    info!("misc {:?}", &hash);
                }
                _ => {
                    println!("read_messages {:?}", &bytes);
                }
            }
        } else {
            info!("not match {:?}", &bytes);
        }
    }
    Ok(())
}

async fn udp_21116(
    mut socket: FramedSocket,
    id_map: Arc<Mutex<HashMap<String, client>>>,
    receiver: Receiver<Event>,
) -> Result<()> {
    loop {
        if let Some(Ok((bytes, addr))) = socket.next().await {
            handle_udp(&mut socket, bytes, addr, id_map.clone(), receiver.clone()).await;
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
    mut socket: &mut FramedSocket,
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
    id_map: std::collections::HashMap<String, std::net::SocketAddr>,
) {
    // let mut msg_out = FetchLocalAddr::new();
    // msg_out.set_fetch_local_addr(msg_out);
    // socket.send(&msg_out, addr).await.ok();
}

async fn udp_send_request_relay(
    mut socket: &mut FramedSocket,
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
    receiver: Receiver<Event>,
) {
    println!("{}", "333");
    if !receiver.is_empty() {
        if let Ok(eve) = receiver.recv().await {
            match eve {
                Event::First(a) => {
                    println!("{}", "first");
                    udp_send_fetch_local_addr(socket, "".to_string(), a).await;
                }
                Event::Second(b) => {
                    println!("{}", "second");
                    udp_send_request_relay(socket, "".to_string(), b).await;
                }
                Event::UnNone => {}
            }
        }
    }
    println!("{}", "222");

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

                    },
                );
                socket.send(&msg_out, addr.clone()).await.ok();
            }
            //完成
            Some(rendezvous_message::Union::register_pk(rp)) => {
                info!("register_pk {:?}", addr);
                let mut msg_out = RendezvousMessage::new();
                msg_out.set_register_pk_response(RegisterPkResponse {
                    result: register_pk_response::Result::OK.into(),
                    ..Default::default()
                });
                socket.send(&msg_out, addr).await.ok();
            }
            //暂存没用
            Some(rendezvous_message::Union::configure_update(rp)) => {
                info!("configure_update {:?}", addr);
                // id_map.insert(rp.id, addr);
                // let mut msg_out = ConfigUpdate::new();
                // msg_out.set_configure_update(msg_out);
                // socket.send(&msg_out, addr).await.ok();
            }
            //暂时没用
            Some(rendezvous_message::Union::software_update(rp)) => {
                info!("software_update {:?}", addr);
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
