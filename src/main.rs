use crate::tokio::select;
use crate::tokio::signal::ctrl_c;
use crate::tokio::time::interval;
use hbb_common::bytes::Bytes;
use hbb_common::message_proto::{message, Message};
use hbb_common::tokio::sync::{mpsc, RwLock};
use hbb_common::{
    allow_err, allow_info,
    anyhow::anyhow,
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
    AddrMangle,
};
use hbb_common::{log, ResultType};
use std::net::SocketAddr;
use std::ops::{Deref, DerefMut};
use std::time::Duration;
use uuid::Uuid;

#[macro_use]
extern crate tracing;

use crate::tokio::sync::Mutex;
use async_channel::{bounded, Receiver, Sender};
use futures::FutureExt;
use hbb_common::futures_util::{SinkExt, StreamExt};
use hbb_common::message_proto::message::Union;
use hbb_common::sodiumoxide::crypto::secretbox;
use hbb_common::sodiumoxide::crypto::stream::stream;
use hbb_common::tokio::net::{TcpStream, UdpSocket};
use hbb_common::tokio::task::JoinHandle;
use hbb_common::tokio_util::codec::{BytesCodec, Framed};
use hbb_common::tokio_util::udp::UdpFramed;
use std::borrow::{Borrow, BorrowMut};
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Error;
use std::sync::Arc;
use tracing_subscriber;

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



#[tokio::main]
async fn main() -> Result<()> {
    let file_appender = tracing_appender::rolling::hourly("/Users/andy/CLionProjects/rustdesk-server-demo", "log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    let collector = tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .init();

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

    tokio::spawn(udp_21116(id_map.clone(), ip_rcv.clone()));
    tokio::spawn(tcp_passive_21117(
        "0.0.0.0:21117",
        id_map.clone(),
        ip_sender.clone(),
    ));
    tokio::spawn(tcp_active_21116("0.0.0.0:21116", id_map, ip_sender.clone()));
    tokio::spawn(tcp_passive_21118(
        "0.0.0.0:21118",
        tx_from_passive,
        rx_from_active.clone(),
    ));
    tokio::spawn(tcp_active_21119(
        "0.0.0.0:21119",
        tx_from_active,
        rx_from_passive.clone(),
    ));

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

async fn tcp_passive_21117(
    addr: &str,
    id_map: Arc<Mutex<HashMap<String, client>>>,
    sender: Sender<Event>,
) -> Result<()> {
    let mut listener_active = new_listener(addr, false).await?;
    loop {
        // Accept the next connection.

        let (stream, addr) = listener_active.accept().await?;

        // Read messages from the client and ignore I/O errors when the client quits.
        tcp_21117_read_rendezvous_message(stream, id_map.clone(), sender.clone()).await;
    }

    Ok(())
}


async fn tcp_21117_read_rendezvous_message(
    mut stream1: TcpStream,
    id_map: Arc<Mutex<HashMap<String, client>>>,
    sender: Sender<Event>,
) -> Result<()> {
    let mut stream = FramedStream::from(stream1);

    if let Some(Ok(bytes)) = stream.next().await {
        if let Ok(msg_in) = RendezvousMessage::parse_from_bytes(&bytes) {
            match msg_in.union {
                Some(rendezvous_message::Union::local_addr(ph)) => {
                    allow_info!(format!("{:?}", &ph));
                    let remote_desk_id = ph.id;
                    let mut id_map = id_map.lock().await;
                    if let Some(client) = id_map.get(&remote_desk_id) {
                        sender.send(Event::Second(client.local_addr)).await;
                    }
                }
                Some(rendezvous_message::Union::relay_response(ph)) => {
                    allow_info!(format!("{:?}", &ph));
                    let mut msg = RendezvousMessage::new();
                    msg.set_relay_response(RelayResponse {
                        relay_server: "47.88.2.164:21117".to_string(),
                        ..Default::default()
                    });

                    stream.send_raw(msg.write_to_bytes().unwrap()).await;
                }
                Some(rendezvous_message::Union::test_nat_request(ph)) => {
                    let mut msg = RendezvousMessage::new();
                    msg.set_test_nat_response(TestNatResponse {
                        port: 0,
                        cu: protobuf::MessageField::some(ConfigUpdate {
                            ..Default::default()
                        }),
                        ..Default::default()
                    });
                    stream.send_raw(msg.write_to_bytes().unwrap()).await;
                }
                Some(rendezvous_message::Union::punch_hole_request(ph)) => {
                    allow_info!(format!("{:?}", &ph));
                    let addr1 = stream.get_ref().local_addr()?;
                    let mut msg = RendezvousMessage::new();
                    msg.set_punch_hole_response(PunchHoleResponse {
                        socket_addr: AddrMangle::encode(addr1),
                        pk: vec![] as Vec<u8>,
                        relay_server: "47.88.2.164:21116".to_string(),
                        union: std::option::Option::Some(punch_hole_response::Union::is_local(
                            false,
                        )),
                        ..Default::default()
                    });
                    if stream.send_raw(msg.write_to_bytes().unwrap()).await.is_ok() {
                        let remote_desk_id = ph.id;
                        let mut id_map = id_map.lock().await;
                        if let Some(client) = id_map.get(&remote_desk_id) {
                            sender.send(Event::First(client.local_addr)).await;
                        }
                    }
                }
                Some(rendezvous_message::Union::request_relay(ph)) => {
                    let mut msg = RendezvousMessage::new();
                    msg.set_relay_response(RelayResponse {
                        socket_addr: vec![],
                        uuid: "".to_string(),
                        relay_server: "".to_string(),
                        refuse_reason: "".to_string(),
                        union: None,
                        unknown_fields: Default::default(),
                        cached_size: Default::default(),
                    });
                    stream.send_raw(msg.write_to_bytes().unwrap()).await;
                }

                _ => {
                    println!("tcp21117_read_rendezvous_message {:?}", &msg_in);
                }
            }
        } else {
            allow_info!(format!("tcp 21117 not match {:?}", &bytes));
        }
    }
    Ok(())
}

async fn tcp_passive_21118(
    addr: &str,
    tx_from_passive: Sender<Vec<u8>>,
    rx_from_active: Receiver<Vec<u8>>,
) -> Result<()> {
    let mut listener_active = new_listener(addr, false).await?;
    loop {
        let (stream, addr) = listener_active.accept().await?;
        println!("{}", "6666");

        // Read messages from the client and ignore I/O errors when the client quits.
        tcp_passive_21118_read_messages(stream, tx_from_passive.clone(), rx_from_active.clone())
            .await?;
    }

    Ok(())
}

async fn tcp_active_21119(
    addr: &str,
    tx_from_active: Sender<Vec<u8>>,
    rx_from_passive: Receiver<Vec<u8>>,
) -> Result<()> {
    let mut listener_active = new_listener(addr, false).await?;
    loop {
        let (stream, addr) = listener_active.accept().await?;

        // Read messages from the client and ignore I/O errors when the client quits.
        tcp_active_21119_read_messages(stream, tx_from_active.clone(), rx_from_passive.clone())
            .await?;
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
                    allow_info!(format!("{:?}", &ph));
                    let remote_desk_id = ph.id;
                    let mut id_map = id_map.lock().await;
                    if let Some(client) = id_map.get(&remote_desk_id) {
                        sender.send(Event::Second(client.local_addr)).await;
                    }
                }
                Some(rendezvous_message::Union::relay_response(ph)) => {
                    allow_info!(format!("{:?}", &ph));
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
                    allow_info!(format!("{:?}", &ph));
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
                    allow_info!(format!("tcp21116_read_rendezvous_message {:?}", &msg_in));
                }
            }
        } else {
            allow_info!(format!("tcp 21116 not match {:?}", &bytes));
        }
    }
    Ok(())
}


async fn tcp_active_21119_read_messages(
    mut stream1: TcpStream,
    tx_from_active: Sender<Vec<u8>>,
    rx_from_passive: Receiver<Vec<u8>>,
) -> Result<()> {
    let mut stream = FramedStream::from(stream1);

    let addr = stream.get_ref().local_addr()?;
    loop {
        select!{

               Ok(bytes) = rx_from_passive.recv() => {
                   if let Ok(msg_in) = Message::parse_from_bytes(&bytes) {
                       match msg_in.union {
                                Some(message::Union::hash(hash)) => {
                                   allow_info!(format!("第一步21119 Receiver hash {:?}", &hash));
                                   let mut msg = Message::new();
                                   msg.set_hash(hash);

                                   stream.send_raw(msg.write_to_bytes().unwrap()).await?;
                                }
                                Some(message::Union::test_delay(hash)) => {
                                   allow_info!(format!("21119 Receiver test_delay {:?}", &hash));
                                   let mut msg = Message::new();
                                   msg.set_test_delay(hash);
                                   stream.send_raw(msg.write_to_bytes().unwrap()).await?;
                               }
                                //完成
                               Some(message::Union::video_frame(hash)) => {

                                   let mut msg = Message::new();
                                   msg.set_video_frame(hash);
                                   stream.send_raw(msg.write_to_bytes().unwrap()).await?;
                               }
                               Some(message::Union::login_response(hash)) => {
                                   allow_info!(format!("21119 Receiver login_response {:?}", &hash));
                                   let mut msg = Message::new();
                                   msg.set_login_response(hash);
                                   stream.send_raw(msg.write_to_bytes().unwrap()).await?;
                               }
                                //完成
                               Some(message::Union::cursor_data(cd)) => {

                                   let mut msg = Message::new();
                                   msg.set_cursor_data(cd);
                                   stream.send_raw(msg.write_to_bytes().unwrap()).await?;
                               }
                               Some(message::Union::cursor_id(id)) => {
                                   allow_info!(format!("21119 Receiver cursor_id{:?}", &id));
                                   // stream.send_raw(id.write_to_bytes().unwrap()).await;
                                   let mut msg = Message::new();
                                   msg.set_cursor_id(id);
                                   stream.send_raw(msg.write_to_bytes().unwrap()).await?;

                               }
                               Some(message::Union::cursor_position(cp)) => {
                                   allow_info!(format!("21119 Receiver cursor_position{:?}", &cp));
                                   let mut msg = Message::new();
                                   msg.set_cursor_position(cp);
                                   stream.send_raw(msg.write_to_bytes().unwrap()).await?;
                               }
                                //完成
                               Some(message::Union::clipboard(cb)) => {
                                   let mut msg = Message::new();
                                   msg.set_clipboard(cb);
                                   stream.send_raw(msg.write_to_bytes().unwrap()).await?;
                               }
                               Some(message::Union::file_response(fr)) => {
                                   allow_info!(format!("21119 Receiver file_response{:?}", &fr));
                                   let mut msg = Message::new();
                                   msg.set_file_response(fr);
                                   stream.send_raw(msg.write_to_bytes().unwrap()).await?;
                               }
                               Some(message::Union::misc(misc)) => {
                                   allow_info!(format!("21119 Receiver misc{:?}", &misc));
                                   let mut msg = Message::new();
                                   msg.set_misc(misc);
                                   stream.send_raw(msg.write_to_bytes().unwrap()).await?;
                               }
                               Some(message::Union::audio_frame(frame)) => {
                                   allow_info!(format!("21119 Receiver audio_frame{:?}", &frame));
                                   let mut msg = Message::new();
                                   msg.set_audio_frame(frame);
                                   stream.send_raw(msg.write_to_bytes().unwrap()).await?;
                               }
                                _ => {
                                    allow_info!(format!("tcp_active_21119  read_messages {:?}", &msg_in));
                                }
                       }
                   }
               }




              Some(Ok(bytes)) =  stream.next_timeout(3000) => {
                if let Ok(msg_in) = Message::parse_from_bytes(&bytes) {
                    match msg_in.union {
                        Some(message::Union::signed_id(hash)) => {
                            allow_info!(format!("active signed_id {:?}", &hash));
                            let mut msg = Message::new();
                            msg.set_signed_id(hash);
                            tx_from_active.send(msg.write_to_bytes().unwrap()).await?;
                        }
                        Some(message::Union::public_key(hash)) => {
                            allow_info!(format!("active public_key {:?}", &hash));
                            let mut msg = Message::new();
                            msg.set_public_key(hash);
                            tx_from_active.send(msg.write_to_bytes().unwrap()).await?;
                        }
                        Some(message::Union::test_delay(hash)) => {
                            allow_info!(format!("active test_delay {:?}", &hash));
                            let mut msg = Message::new();
                            msg.set_test_delay(hash);
                            tx_from_active.send(msg.write_to_bytes().unwrap()).await?;
                        }
                        Some(message::Union::video_frame(hash)) => {
                            let mut msg = Message::new();
                            msg.set_video_frame(hash);
                            tx_from_active.send(msg.write_to_bytes().unwrap()).await?;
                        }
                        Some(message::Union::login_request(hash)) => {
                            allow_info!(format!("active login_request {:?}", &hash));
                            let mut msg = Message::new();
                            msg.set_login_request(hash);
                            tx_from_active.send(msg.write_to_bytes().unwrap()).await?;
                        }
                        Some(message::Union::login_response(hash)) => {
                            allow_info!(format!("active login_response {:?}", &hash));
                            let mut msg = Message::new();
                            msg.set_login_response(hash);
                            tx_from_active.send(msg.write_to_bytes().unwrap()).await?;
                        }

                        Some(message::Union::mouse_event(hash)) => {
                            allow_info!(format!("active mouse_event {:?}", &hash));
                            let mut msg = Message::new();
                            msg.set_mouse_event(hash);
                            tx_from_active.send(msg.write_to_bytes().unwrap()).await?;
                        }
                        Some(message::Union::audio_frame(hash)) => {
                            allow_info!(format!("active audio_frame {:?}", &hash));
                            let mut msg = Message::new();
                            msg.set_audio_frame(hash);
                            tx_from_active.send(msg.write_to_bytes().unwrap()).await?;
                        }
                        Some(message::Union::cursor_data(hash)) => {
                            let mut msg = Message::new();
                            msg.set_cursor_data(hash);
                            tx_from_active.send(msg.write_to_bytes().unwrap()).await?;
                        }
                        Some(message::Union::cursor_position(hash)) => {
                            allow_info!(format!("active cursor_position {:?}", &hash));
                            let mut msg = Message::new();
                            msg.set_cursor_position(hash);
                            tx_from_active.send(msg.write_to_bytes().unwrap()).await?;
                        }
                        Some(message::Union::cursor_id(hash)) => {
                            allow_info!(format!("active cursor_id{:?}", &hash));
                            let mut msg = Message::new();
                            msg.set_cursor_id(hash);
                            tx_from_active.send(msg.write_to_bytes().unwrap()).await?;
                        }

                        Some(message::Union::key_event(hash)) => {
                            allow_info!(format!("active key_event {:?}", &hash));
                            let mut msg = Message::new();
                            msg.set_key_event(hash);
                            tx_from_active.send(msg.write_to_bytes().unwrap()).await?;
                        }
                        Some(message::Union::clipboard(hash)) => {
                            let mut msg = Message::new();
                            msg.set_clipboard(hash);
                            tx_from_active.send(msg.write_to_bytes().unwrap()).await?;
                        }
                        Some(message::Union::file_action(hash)) => {
                            allow_info!(format!("active file_action {:?}", &hash));
                            let mut msg = Message::new();
                            msg.set_file_action(hash);
                            tx_from_active.send(msg.write_to_bytes().unwrap()).await?;
                        }
                        Some(message::Union::file_response(hash)) => {
                            allow_info!(format!("active file_response {:?}", &hash));
                            let mut msg = Message::new();
                            msg.set_file_response(hash);
                            tx_from_active.send(msg.write_to_bytes().unwrap()).await?;
                        }
                        Some(message::Union::misc(hash)) => {
                            allow_info!(format!("active misc {:?}", &hash));
                            let mut msg = Message::new();
                            msg.set_misc(hash);
                            tx_from_active.send(msg.write_to_bytes().unwrap()).await?;
                        }
                        _ => {
                            allow_info!(format!("tcp_active_21119  read_messages {:?}", &msg_in));
                        }
                    }
                }
    }
        }
    }


    Ok(())
}


async fn tcp_passive_21118_read_messages(
    mut stream1: TcpStream,
    tx_from_passive: Sender<Vec<u8>>,
    rx_from_active: Receiver<Vec<u8>>,
) -> Result<()> {
    let mut stream = FramedStream::from(stream1);

    loop {
        select! {
                   Ok(bytes) = rx_from_active.recv() => {
                         if let Ok(msg_in) = Message::parse_from_bytes(&bytes){
                            match msg_in.union {
                                //完成
                                Some(message::Union::login_request(hash)) => {
                                    allow_info!(format!("21118 Receiver login_request {:?}", &hash));
                                    let mut msg = Message::new();
                                    msg.set_login_request(hash);
                                    stream.send_raw(msg.write_to_bytes().unwrap()).await?;
                                }
                                Some(message::Union::test_delay(hash)) => {
                                allow_info!(format!("21118 Receiver test_delay {:?}", &hash));
                                let mut msg = Message::new();
                                msg.set_test_delay(hash);
                                stream.send_raw(msg.write_to_bytes().unwrap()).await?;
                                }
                                Some(message::Union::video_frame(hash)) => {

                                    let mut msg = Message::new();
                                    msg.set_video_frame(hash);
                                    stream.send_raw(msg.write_to_bytes().unwrap()).await?;
                                }
                                Some(message::Union::login_response(hash)) => {
                                    allow_info!(format!("21118 Receiver login_response {:?}", &hash));
                                    let mut msg = Message::new();
                                    msg.set_login_response(hash);
                                    stream.send_raw(msg.write_to_bytes().unwrap()).await?;
                                }

                                Some(message::Union::cursor_data(cd)) => {

                                    let mut msg = Message::new();
                                    msg.set_cursor_data(cd);
                                    stream.send_raw(msg.write_to_bytes().unwrap()).await?;
                                }
                                Some(message::Union::cursor_id(id)) => {
                                    allow_info!(format!("21118 Receiver cursor_id{:?}", &id));
                                    let mut msg = Message::new();
                                    msg.set_cursor_id(id);
                                    stream.send_raw(msg.write_to_bytes().unwrap()).await?;
                                    // stream.send_raw(id.write_to_bytes().unwrap()).await;
                                }
                                Some(message::Union::cursor_position(cp)) => {
                                    allow_info!(format!("21118 Receiver cursor_position{:?}", &cp));
                                    let mut msg = Message::new();
                                    msg.set_cursor_position(cp);
                                    stream.send_raw(msg.write_to_bytes().unwrap()).await?;
                                }
                                //完成
                                Some(message::Union::clipboard(cb)) => {

                                    let mut msg = Message::new();
                                    msg.set_clipboard(cb);
                                    stream.send_raw(msg.write_to_bytes().unwrap()).await?;
                                }
                                //暂存
                                Some(message::Union::file_response(fr)) => {
                                    allow_info!(format!("21118 Receiver file_response{:?}", &fr));
                                    let mut msg = Message::new();
                                    msg.set_file_response(fr);
                                    stream.send_raw(msg.write_to_bytes().unwrap()).await?;
                                }
                                Some(message::Union::misc(misc)) => {
                                    allow_info!(format!("21118 Receiver misc{:?}", &misc));
                                    let mut msg = Message::new();
                                    msg.set_misc(misc);
                                    stream.send_raw(msg.write_to_bytes().unwrap()).await?;
                                }
                                Some(message::Union::audio_frame(frame)) => {
                                    allow_info!(format!("21118 Receiver audio_frame{:?}", &frame));
                                    let mut msg = Message::new();
                                    msg.set_audio_frame(frame);
                                    stream.send_raw(msg.write_to_bytes().unwrap()).await?;
                                }


                                Some(message::Union::file_action(fa)) =>{
                                    allow_info!(format!("21118 Receiver file_action{:?}", &fa));
                                    let mut msg = Message::new();
                                    msg.set_file_action(fa);
                                    stream.send_raw(msg.write_to_bytes().unwrap()).await?;
                                }
                                //完成
                                Some(message::Union::key_event(mut me)) =>{
                                    let mut msg = Message::new();
                                    msg.set_key_event(me);
                                    stream.send_raw(msg.write_to_bytes().unwrap()).await?;
                                }
                                Some(message::Union::mouse_event(frame)) => {
                                    allow_info!(format!("21118 Receiver audio_frame{:?}", &frame));

                                    let mut msg = Message::new();
                                    msg.set_mouse_event(frame);
                                    stream.send_raw(msg.write_to_bytes().unwrap()).await?;
                                }
                                _ => {
                                    allow_info!(format!("tcp_active_21118  read_messages {:?}", &msg_in));
                                }

                                }
                            }
                        }
                    Some(Ok(bytes)) =  stream.next_timeout(3000) =>  {
                        if let Ok(msg_in) = Message::parse_from_bytes(&bytes){
                            match msg_in.union{
                                 Some(message::Union::signed_id(hash)) => {
                                allow_info!(format!("passive signed_id {:?}", &hash));
                                let mut msg = Message::new();
                                msg.set_signed_id(hash);
                                tx_from_passive.send(msg.write_to_bytes().unwrap()).await?;
                                    }
                                Some(message::Union::public_key(hash)) =>{
                                      allow_info!(format!("passive public_key {:?}", &hash));
                                    let mut msg = Message::new();
                                    msg.set_public_key(hash);
                                    tx_from_passive.send(msg.write_to_bytes().unwrap()).await?;
                                }
                                        //             //被动端转发延时请求给主动方
                                Some(message::Union::test_delay(hash)) => {
                                    allow_info!(format!("passive test_delay {:?}", &hash));
                                    let mut msg = Message::new();
                                    msg.set_test_delay(hash);
                                    tx_from_passive.send(msg.write_to_bytes().unwrap()).await?;
                                }
                                Some(message::Union::video_frame(hash)) => {

                                    let mut msg = Message::new();
                                    msg.set_video_frame(hash);
                                    tx_from_passive.send(msg.write_to_bytes().unwrap()).await?;
                                }
                                    Some(message::Union::login_request(hash)) => {
                                        allow_info!(format!("passive login_request {:?}", &hash));
                                        let mut msg = Message::new();
                                        msg.set_login_request(hash);
                                        tx_from_passive.send(msg.write_to_bytes().unwrap()).await?;
                                    }
                                    Some(message::Union::login_response(hash)) => {
                                        allow_info!(format!("++++++++++++++++jjjjjj-1111  21118 login_response {:?}", &hash));
                                        let mut msg = Message::new();
                                        msg.set_login_response(hash);
                                        tx_from_passive.send(msg.write_to_bytes().unwrap()).await?;
                                    }
                                    //完成
                                    Some(message::Union::hash(hash)) => {
                                        allow_info!(format!("passive hash {:?}", &hash));
                                        let mut msg = Message::new();
                                           msg.set_hash(hash);
                                        tx_from_passive.send(msg.write_to_bytes().unwrap()).await?;
                                    }
                                    Some(message::Union::mouse_event(hash)) => {
                                        allow_info!(format!("passive mouse_event {:?}", &hash));
                                        let mut msg = Message::new();
                                        msg.set_mouse_event(hash);
                                        tx_from_passive.send(msg.write_to_bytes().unwrap()).await?;
                                    }
                                    Some(message::Union::audio_frame(hash)) => {
                                        allow_info!(format!("passive audio_frame {:?}", &hash));
                                        let mut msg = Message::new();
                                        msg.set_audio_frame(hash);
                                        tx_from_passive.send(msg.write_to_bytes().unwrap()).await?;
                                    }
                                    Some(message::Union::cursor_data(hash)) => {

                                        let mut msg = Message::new();
                                        msg.set_cursor_data(hash);
                                        tx_from_passive.send(msg.write_to_bytes().unwrap()).await?;
                                    }
                                    Some(message::Union::cursor_position(hash)) => {
                                        allow_info!(format!("passive cursor_position {:?}", &hash));
                                        let mut msg = Message::new();
                                        msg.set_cursor_position(hash);
                                        tx_from_passive.send(msg.write_to_bytes().unwrap()).await?;
                                    }
                                    Some(message::Union::cursor_id(hash)) => {
                                    allow_info!(format!("passive cursor_id {:?}", &hash));
                                    // tx_from_passive.send(hash.write_to_bytes().unwrap()).await;
                                    let mut msg = Message::new();
                                    msg.set_cursor_id(hash);
                                    tx_from_passive.send(msg.write_to_bytes().unwrap()).await?;

                                    }
                                    Some(message::Union::key_event(hash)) => {
                                        allow_info!(format!("passive key_event {:?}", &hash));
                                        let mut msg = Message::new();
                                        msg.set_key_event(hash);
                                        tx_from_passive.send(msg.write_to_bytes().unwrap()).await?;
                                    }
                                    Some(message::Union::clipboard(hash)) => {
                                      
                                        let mut msg = Message::new();
                                        msg.set_clipboard(hash);
                                        tx_from_passive.send(msg.write_to_bytes().unwrap()).await?;
                                    }
                                    Some(message::Union::file_action(hash)) => {
                                        allow_info!(format!("passive file_action {:?}", &hash));
                                        let mut msg = Message::new();
                                        msg.set_file_action(hash);
                                        tx_from_passive.send(msg.write_to_bytes().unwrap()).await?;
                                    }
                                    Some(message::Union::file_response(hash)) => {
                                        allow_info!(format!("passive file_response {:?}", &hash));
                                        let mut msg = Message::new();
                                        msg.set_file_response(hash);
                                        tx_from_passive.send(msg.write_to_bytes().unwrap()).await?;
                                    }
                                    Some(message::Union::misc(hash)) => {
                                        allow_info!(format!("passive misc {:?}", &hash));
                                        let mut msg = Message::new();
                                        msg.set_misc(hash);
                                        tx_from_passive.send(msg.write_to_bytes().unwrap()).await?;
                                    }
                                //完成
                                 _ => {
                                allow_info!(format!("tcp_passive_21118  read_messages {:?}", &msg_in));
                            }
                            }
                        }
                    }

                    }
    }
}



async fn udp_21116(
    id_map: Arc<Mutex<HashMap<String, client>>>,
    receiver: Receiver<Event>,
) -> Result<()> {
    let mut socket1 = UdpSocket::bind(to_socket_addr("0.0.0.0:21116").unwrap()).await?;

    let mut r = Arc::new(socket1);
    let mut s = r.clone();
    let mut socket = UdpFramed::new(r, BytesCodec::new());
    let mut socket1 = UdpFramed::new(s, BytesCodec::new());

    let receiver1 = receiver.clone();

    tokio::spawn(async move {
        while let Ok(eve) = receiver1.recv().await {
            match eve {
                Event::First(a) => {
                    allow_info!(format!("{}", "first"));
                    udp_send_fetch_local_addr(&mut socket1, "".to_string(), a).await;
                }
                Event::Second(b) => {
                    allow_info!(format!("{}", "second"));

                    udp_send_request_relay(&mut socket1, "".to_string(), b).await;
                }
                Event::UnNone => {}
            }
        }
    });

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
    socket: &mut UdpFramed<BytesCodec, Arc<UdpSocket>>,
    relay_server: String,
    addr: std::net::SocketAddr,
) -> Result<()> {
    let mut msg = RendezvousMessage::new();
    let addr1 = to_socket_addr("47.88.2.164:21117").unwrap();

    let vec1 = AddrMangle::encode(addr1);
    msg.set_fetch_local_addr(FetchLocalAddr {
        socket_addr: vec1,
        relay_server,
        ..Default::default()
    });

    socket
        .send((Bytes::from(msg.write_to_bytes().unwrap()), addr))
        .await?;

    Ok(())
}

async fn udp_send_punch_hole(
    socket: &mut UdpFramed<BytesCodec, Arc<UdpSocket>>,
    bytes: BytesMut,
    addr: std::net::SocketAddr,
    id_map: &mut std::collections::HashMap<String, std::net::SocketAddr>,
) {
    // let mut msg_out = FetchLocalAddr::new();
    // msg_out.set_fetch_local_addr(msg_out);
    // socket.send(&msg_out, addr).await.ok();
}

async fn udp_send_configure_update(
    socket: &mut UdpFramed<BytesCodec, Arc<UdpSocket>>,
    bytes: BytesMut,
    addr: std::net::SocketAddr,
    id_map: std::collections::HashMap<String, std::net::SocketAddr>,
) {
    // let mut msg_out = FetchLocalAddr::new();
    // msg_out.set_fetch_local_addr(msg_out);
    // socket.send(&msg_out, addr).await.ok();
}

async fn udp_send_request_relay(
    socket: &mut UdpFramed<BytesCodec, Arc<UdpSocket>>,
    relay_server: String,
    addr: std::net::SocketAddr,
) -> Result<()> {
    let mut msg = RendezvousMessage::new();

    msg.set_request_relay(RequestRelay {
        relay_server,
        ..Default::default()
    });
    //通过udp回复客户端打洞请求,提供一个中继cdn地址

    socket
        .send((Bytes::from(msg.write_to_bytes().unwrap()), addr))
        .await?;
    Ok(())
}

async fn handle_udp(
    socket: &mut UdpFramed<BytesCodec, Arc<UdpSocket>>,
    bytes: BytesMut,
    addr: std::net::SocketAddr,
    id_map: Arc<Mutex<HashMap<String, client>>>,
    receiver: Receiver<Event>,
) {
    if let Ok(msg_in) = RendezvousMessage::parse_from_bytes(&bytes) {
        match msg_in.union {
            //不停的register_peer保持心跳,检测心跳告诉对方不在线
            Some(rendezvous_message::Union::register_peer(rp)) => {
                allow_info!(format!("register_peer {:?}", &addr));
                let mut msg = RendezvousMessage::new();
                let mut id_map = id_map.lock().await;
                if let Some(client) = id_map.get(&rp.id) {
                    msg.set_register_peer_response(RegisterPeerResponse {
                        request_pk: false,
                        ..Default::default()
                    });
                } else {
                    msg.set_register_peer_response(RegisterPeerResponse {
                        request_pk: true,
                        ..Default::default()
                    });
                };
                id_map.insert(
                    rp.id,
                    client {
                        timestamp: get_time() as u64,
                        local_addr: addr,
                    },
                );
                socket
                    .send((Bytes::from(msg.write_to_bytes().unwrap()), addr.clone()))
                    .await;
            }
            //完成
            Some(rendezvous_message::Union::register_pk(rp)) => {
                allow_info!(format!("register_pk {:?}", addr));
                let mut msg = RendezvousMessage::new();
                msg.set_register_pk_response(RegisterPkResponse {
                    result: register_pk_response::Result::OK.into(),
                    ..Default::default()
                });

                socket
                    .send((Bytes::from(msg.write_to_bytes().unwrap()), addr.clone()))
                    .await;
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
                allow_info!(format!("software_update {:?}", addr));
                // id_map.insert(rp.id, addr);
                // let mut msg_out = SoftwareUpdate::new();
                // msg_out.set_software_update(msg_out);
                // socket.send(&msg_out, addr).await.ok();
            }

            _ => {
                allow_info!("不匹配的指令");
            }
        }
    }
}
