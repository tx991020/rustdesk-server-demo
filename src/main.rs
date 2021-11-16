use crate::tokio::select;
use crate::tokio::signal::ctrl_c;
use crate::tokio::time::interval;
use hbb_common::bytes::Bytes;
use hbb_common::message_proto::{message, Message};
use hbb_common::tokio::sync::{broadcast, mpsc, Mutex, RwLock};
use hbb_common::tokio::time;
use hbb_common::{
    allow_err, allow_info,
    anyhow::{bail, Context, Result},
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
use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::ops::{Deref, DerefMut};
use std::time::Duration;
use uuid::Uuid;

#[macro_use]
extern crate tracing;

use async_channel::{bounded, unbounded, Receiver, Sender};
use dashmap::mapref::one::Ref;
use dashmap::DashMap;
use futures::FutureExt;
use hbb_common::futures_util::{SinkExt, StreamExt};
use hbb_common::message_proto::message::Union;
use hbb_common::message_proto::misc::Union::option;
use hbb_common::sodiumoxide::crypto::secretbox;
use hbb_common::sodiumoxide::crypto::stream::stream;
use hbb_common::tokio::net::{TcpListener, TcpStream, UdpSocket};
use hbb_common::tokio::sync::mpsc::UnboundedSender;
use hbb_common::tokio::task::JoinHandle;
use hbb_common::tokio_util::codec::{BytesCodec, Framed, LengthDelimitedCodec};
use hbb_common::tokio_util::udp::UdpFramed;
use smol::Timer;
use std::borrow::{Borrow, BorrowMut};
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Error;
use std::sync::Arc;
use tracing_subscriber;

/// Shorthand for the transmit half of the message channel.
type Tx = Sender<Vec<u8>>;

/// Shorthand for the receive half of the message channel.
type Rx = Receiver<Vec<u8>>;

struct Shared {
    receivers_16: HashMap<SocketAddr, Rx>,
    receivers_17: HashMap<SocketAddr, Rx>,
    kv16: HashMap<String, SocketAddr>,
    kv17: HashMap<String, SocketAddr>,
    status: HashMap<SocketAddr, i8>,
}

impl Shared {
    /// Create a new, empty, instance of `Shared`.
    fn new() -> Self {
        Shared {
            receivers_16: HashMap::new(),
            receivers_17: HashMap::new(),
            status: HashMap::new(),
            kv16: HashMap::new(),
            kv17: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct client {
    //心跳
    timestamp: u64,
    local_addr: SocketAddr,
    peer_ip: String,
    uuid: String,
}

#[derive(Debug, Clone)]
enum Event {
    /// 打洞第一步
    First(String, SocketAddr),

    ///打洞第二步
    Second(String, SocketAddr),
    UnNone,
}

//pub const RENDEZVOUS_SERVER: &'static str = "39.107.33.253:21117";


lazy_static::lazy_static! {
    pub static ref RENDEZVOUS_SERVER: String = std::env::var("RENDEZVOUS_SERVER").unwrap();
    pub static ref IdMap: Arc<DashMap<String, client>> = Arc::new(DashMap::new());
      pub static ref IpMap: Arc<DashMap<String, client>> = Arc::new(DashMap::new());

}

//默认情况下，hbbs 侦听 21115(tcp) 和 21116(tcp/udp)，hbbr 侦听 21117(tcp)。请务必在防火墙中打开这些端口
//上线离线问题, id,ip
pub fn get_time() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0) as _
}

//1处理配对关系
//2 转发消息 ws， proxy
//3离开是,删除关系

#[tokio::main]
async fn main() -> Result<()> {
    // let file_appender = tracing_appender::rolling::hourly("/Users/andy/CLionProjects/rustdesk-server-demo", "log");
    // let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    // tracing_subscriber::fmt()
    //     .with_writer(non_blocking)
    //     .init();

    tracing_subscriber::fmt::init();

    let (ip_sender, mut ip_rcv) = async_channel::unbounded::<Event>();
    let state = Arc::new(Mutex::new(Shared::new()));
    tokio::spawn(traverse_id_map());
    tokio::spawn(traverse_ip_map());
    tokio::spawn(udp_21116(ip_rcv.clone()));
    tokio::spawn(tcp_passive_21117(
        "0.0.0.0:21117",
        state.clone(),
        ip_sender.clone(),
    ));
    //完成
    tokio::spawn(tcp_active_21116(
        "0.0.0.0:21116",
        state.clone(),
        ip_sender.clone(),
    ));

    ctrl_c().await?;
    Ok(())
}

//删除30秒内没心跳的号
async fn traverse_id_map() {
    let mut interval = time::interval(Duration::from_secs(30));
    loop {
        let mut guard = IdMap.clone();
        println!("在线用户{:#?}", &guard);
        guard.retain(|key, value| value.timestamp > get_time() - 1000 * 20);
        drop(guard);
        interval.tick().await;
    }
}

//删除30秒内没心跳的号
async fn traverse_ip_map() {
    let mut interval = time::interval(Duration::from_secs(30));
    loop {
        let mut guard = IdMap.clone();
        println!("在线用户{:#?}", &guard);
        guard.retain(|key, value| value.timestamp > get_time() - 1000 * 20);
        drop(guard);
        interval.tick().await;
    }
}

async fn tcp_active_21116(
    addr: &str,
    state: Arc<Mutex<Shared>>,
    sender: Sender<Event>,
) -> Result<()> {
    let mut listener_active = new_listener(addr, false).await?;
    loop {
        let (stream, addr1) = listener_active.accept().await?;

        tokio::spawn(tcp_21116_read_rendezvous_message(
            stream,
            state.clone(),
            sender.clone(),
            addr1,
        ));
    }

    Ok(())
}

async fn tcp_passive_21117(
    addr: &str,
    state: Arc<Mutex<Shared>>,
    sender: Sender<Event>,
) -> Result<()> {
    let mut listener_active = new_listener(addr, false).await?;
    loop {
        let (stream, addr1) = listener_active.accept().await?;

        // Read messages from the client and ignore I/O errors when the client quits.
        tokio::spawn(tcp_21117_read_rendezvous_message(
            stream,
            state.clone(),
            sender.clone(),
            addr1,
        ));
    }

    Ok(())
}

//被动tcp方
async fn tcp_21117_read_rendezvous_message(
    mut stream1: TcpStream,
    state: Arc<Mutex<Shared>>,
    sender: Sender<Event>,
    addr: SocketAddr,
) -> Result<()> {
    let mut stream = FramedStream::from(stream1);

    let mut step = 0;
    let guard = state.lock().await;
    let mut opt = guard.status.get(&addr);
    if opt.is_some() {
        step = 1
    }
    drop(guard);

    let (tx, mut rx) = unbounded::<Vec<u8>>();
    if step == 0 {
        loop {
            if let Some(Ok(bytes)) = stream.next_timeout(3000).await {
                if let Ok(msg_in) = RendezvousMessage::parse_from_bytes(&bytes) {
                    match msg_in.union {
                        Some(rendezvous_message::Union::local_addr(ph)) => {
                            let remote_desk_id = ph.id;
                            let mut id_map = IdMap.clone();
                            if let Some(client) = id_map.get(&remote_desk_id) {
                                sender
                                    .send(Event::Second(remote_desk_id, client.local_addr))
                                    .await;
                            };
                        }
                        //测试能否与中继ping通
                        Some(rendezvous_message::Union::relay_response(ph)) => {
                            allow_info!(format!("{:?}", &ph));
                            let mut msg = RendezvousMessage::new();
                            msg.set_relay_response(RelayResponse {
                                relay_server: "39.107.33.253:21117".to_string(),
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

                        Some(rendezvous_message::Union::request_relay(ph)) => {
                            allow_info!(format!(
                                "被动方 290 uuid {:?}, {:?},{:?}",
                                &ph.uuid, &ph.id, &addr
                            ));

                            let mut msg = RendezvousMessage::new();
                            msg.set_relay_response(RelayResponse {
                                socket_addr: vec![],
                                uuid: ph.uuid.clone(),
                                relay_server: "".to_string(),
                                refuse_reason: "".to_string(),
                                union: None,
                                unknown_fields: Default::default(),
                                cached_size: Default::default(),
                            });
                            stream.send_raw(msg.write_to_bytes().unwrap()).await?;

                            let mut s = state.lock().await;
                            s.kv17.insert(ph.id, addr);
                            s.status.insert(addr, 1);
                            s.receivers_16.insert(addr, rx.clone());
                            drop(s);
                            info!("{}", "301 finish");
                            break;
                        }

                        _ => {
                            println!("tcp21117_read_rendezvous_message {:?}", &msg_in);
                        }
                    }
                }
            } else {
                info!("tcp 21117  step0 连接超时");
                break;
            }
        }
    }
    sleep(2 as f32).await;

    let (tx1, mut rx1) = unbounded::<Vec<u8>>();
    println!("21117 AAAAAAAAAAAAAA{:?}", &addr);
    //给lock加作用域
    let mut re = None;
    {
        let mut s = state.lock().await;

        println!("21117 1111111111{:?},{:#?},{:#?}", &addr, s.kv16, s.kv17);

        let mut key = "";
        for (k, v) in s.kv17.iter() {
            if v == &addr {
                key = k
            }
        }
        info!("21117 key ---------{},kv16{:?}", key, s.kv16);
        let host = s.kv16.get(key).context("not found")?;
        if let Some(r) = s.receivers_17.get(host) {
            re = Some(r.clone());
            drop(s);
        };
    }
    if re.is_some() {
        println!("{}", "MMMMMMMMMMMMMMMMMMMM  message from 21118");
        tokio::spawn(fx1(tx1.clone(), re.unwrap()));
    };
    info!("{}", "2117 344444444444");

    loop {
        select! {

           Ok(bytes) = rx1.recv() => {
            println!("{}", "21117 ---------------- recv");
               if let Ok(msg_in) = Message::parse_from_bytes(&bytes) {
                   match msg_in.union {
                            //完成
                        Some(message::Union::cmd_action(hash)) => {
                        println!("45666666 ----------------{:?}", &hash);
                        allow_info!(format!("21117 signed_id {:?}", &hash));
                        let mut msg = Message::new();
                        msg.set_cmd_action(hash);
                        stream.send_raw(msg.write_to_bytes().unwrap()).await;
                        }
                            Some(message::Union::login_request(hash)) => {
                                allow_info!(format!("21117 Receiver login_request {:?}", &hash));
                                let mut msg = Message::new();
                                msg.set_login_request(hash);
                                stream.send_raw(msg.write_to_bytes().unwrap()).await;
                            }
                            Some(message::Union::test_delay(hash)) => {
                               allow_info!(format!("21117 Receiver test_delay {:?}", &hash));
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
                               allow_info!(format!("21117 Receiver login_response {:?}", &hash));
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
                               allow_info!(format!("21117 Receiver cursor_id{:?}", &id));
                               // stream.send_raw(id.write_to_bytes().unwrap()).await;
                               let mut msg = Message::new();
                               msg.set_cursor_id(id);
                               stream.send_raw(msg.write_to_bytes().unwrap()).await?;

                           }
                           Some(message::Union::cursor_position(cp)) => {

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

                               let mut msg = Message::new();
                               msg.set_file_response(fr);
                               stream.send_raw(msg.write_to_bytes().unwrap()).await?;
                           }
                           Some(message::Union::misc(misc)) => {

                               let mut msg = Message::new();
                               msg.set_misc(misc);
                               stream.send_raw(msg.write_to_bytes().unwrap()).await?;
                           }
                           Some(message::Union::audio_frame(frame)) => {

                               let mut msg = Message::new();
                               msg.set_audio_frame(frame);
                               stream.send_raw(msg.write_to_bytes().unwrap()).await?;
                           }
                          Some(message::Union::file_action(fa)) =>{

                                    let mut msg = Message::new();
                                    msg.set_file_action(fa);
                                    stream.send_raw(msg.write_to_bytes().unwrap()).await;
                                }
                         Some(message::Union::key_event(mut me)) =>{
                                    let mut msg = Message::new();
                                    msg.set_key_event(me);
                                    stream.send_raw(msg.write_to_bytes().unwrap()).await;
                                }
                          Some(message::Union::mouse_event(frame)) => {


                                    let mut msg = Message::new();
                                    msg.set_mouse_event(frame);
                                    stream.send_raw(msg.write_to_bytes().unwrap()).await;
                                }
                            _ => {
                                allow_info!(format!("tcp_21117  read_messages {:?}", &msg_in));
                            }
                   }
               }
           }




          res =  stream.next_timeout(30000) => {
            if let Some(Ok(bytes)) =res {
            if let Ok(msg_in) = Message::parse_from_bytes(&bytes) {
                match msg_in.union {
                            //21117 send
                     Some(message::Union::cmd_action(hash)) => {
                        println!("45666666 ----------------{:?}", &hash);
                        allow_info!(format!("21117 signed_id {:?}", &hash));
                        let mut msg = Message::new();
                            msg.set_cmd_action(hash);
                         tx.send(msg.write_to_bytes().unwrap()).await?;
                        }

                    Some(message::Union::signed_id(hash)) => {
                        allow_info!(format!("21117 signed_id {:?}", &hash));
                        let mut msg = Message::new();
                        msg.set_signed_id(hash);
                        tx.send(msg.write_to_bytes().unwrap()).await?;
                    }
                    Some(message::Union::public_key(hash)) => {
                        allow_info!(format!("21117 public_key {:?}", &hash));
                        let mut msg = Message::new();
                        msg.set_public_key(hash);
                        tx.send(msg.write_to_bytes().unwrap()).await?;
                    }
                    Some(message::Union::test_delay(hash)) => {
                        allow_info!(format!("21117 test_delay {:?}", &hash));
                        let mut msg = Message::new();
                        msg.set_test_delay(hash);
                        tx.send(msg.write_to_bytes().unwrap()).await?;
                    }
                    Some(message::Union::video_frame(hash)) => {
                        let mut msg = Message::new();
                        msg.set_video_frame(hash);
                        tx.send(msg.write_to_bytes().unwrap()).await?;
                    }
                    Some(message::Union::login_request(hash)) => {
                        allow_info!(format!("21117 login_request {:?}", &hash));
                        let mut msg = Message::new();
                        msg.set_login_request(hash);
                        tx.send(msg.write_to_bytes().unwrap()).await?;
                    }
                    Some(message::Union::login_response(hash)) => {
                        allow_info!(format!("21117 login_response {:?}", &hash));
                        let mut msg = Message::new();
                        msg.set_login_response(hash);
                        tx.send(msg.write_to_bytes().unwrap()).await?;
                    }
                    //完成
                    Some(message::Union::hash(hash)) => {
                        allow_info!(format!("21117 hash {:?}", &hash));
                        let mut msg = Message::new();
                           msg.set_hash(hash);
                        tx.send(msg.write_to_bytes().unwrap()).await;
                    }

                    Some(message::Union::mouse_event(hash)) => {

                        let mut msg = Message::new();
                        msg.set_mouse_event(hash);
                        tx.send(msg.write_to_bytes().unwrap()).await?;
                    }
                    Some(message::Union::audio_frame(hash)) => {

                        let mut msg = Message::new();
                        msg.set_audio_frame(hash);
                        tx.send(msg.write_to_bytes().unwrap()).await?;
                    }
                    Some(message::Union::cursor_data(hash)) => {
                        let mut msg = Message::new();
                        msg.set_cursor_data(hash);
                        tx.send(msg.write_to_bytes().unwrap()).await?;
                    }
                    Some(message::Union::cursor_position(hash)) => {

                        let mut msg = Message::new();
                        msg.set_cursor_position(hash);
                        tx.send(msg.write_to_bytes().unwrap()).await?;
                    }
                    Some(message::Union::cursor_id(hash)) => {
                        allow_info!(format!("21117 cursor_id{:?}", &hash));
                        let mut msg = Message::new();
                        msg.set_cursor_id(hash);
                        tx.send(msg.write_to_bytes().unwrap()).await?;
                    }

                    Some(message::Union::key_event(hash)) => {

                        let mut msg = Message::new();
                        msg.set_key_event(hash);
                        tx.send(msg.write_to_bytes().unwrap()).await?;
                    }
                    Some(message::Union::clipboard(hash)) => {
                        let mut msg = Message::new();
                        msg.set_clipboard(hash);
                        tx.send(msg.write_to_bytes().unwrap()).await?;
                    }
                    Some(message::Union::file_action(hash)) => {

                        let mut msg = Message::new();
                        msg.set_file_action(hash);
                        tx.send(msg.write_to_bytes().unwrap()).await?;
                    }
                    Some(message::Union::file_response(hash)) => {

                        let mut msg = Message::new();
                        msg.set_file_response(hash);
                        tx.send(msg.write_to_bytes().unwrap()).await?;
                    }
                    Some(message::Union::misc(hash)) => {

                        let mut msg = Message::new();
                        msg.set_misc(hash);
                        tx.send(msg.write_to_bytes().unwrap()).await?;
                    }
                    _ => {
                        allow_info!(format!("tcp_21117  read_messages {:?}", &msg_in));
                    }
                }
            }
                }else {
                info!("tcp 21117连接超时");
                break;
            }
        }
         _ = Timer::after(Duration::from_secs(5)) => {
               info!("ticker 21117连接超时");
                break;
            }


        }
    }
    info!("drop stream  21117");
    drop(stream);

    Ok(())
}

async fn tcp_21116_read_rendezvous_message(
    mut stream: TcpStream,
    state: Arc<Mutex<Shared>>,
    sender: Sender<Event>,
    addr: SocketAddr,
) -> Result<()> {
    let mut stream = FramedStream::from(stream);
    let mut step = 0;
    let guard = state.lock().await;
    let mut opt = guard.status.get(&addr);
    if opt.is_some() {
        step = 1
    }
    drop(guard);

    let (tx, mut rx) = unbounded::<Vec<u8>>();
    if step == 0 {
        loop {
            if let Some(Ok(bytes)) = stream.next_timeout(30000).await {
                if let Ok(msg_in) = RendezvousMessage::parse_from_bytes(&bytes) {
                    match msg_in.union {
                        Some(rendezvous_message::Union::relay_response(ph)) => {
                            allow_info!(format!("{:?}", &ph));
                            let mut msg_out = RendezvousMessage::new();
                            msg_out.set_relay_response(RelayResponse {
                                relay_server: "39.107.33.253:21117".to_string(),
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
                        //主动发打洞第一步
                        //如果对方不在线，直接报错告诉主动方
                        Some(rendezvous_message::Union::punch_hole_request(ph)) => {
                            info!("{}", "---------punch_hole_request 21116");
                            let remote_desk_id = ph.id;
                            let mut id_map = IdMap.clone();
                            let client = id_map.get(&remote_desk_id).context("not found");

                            if client.is_err() {
                                let mut msg_out = RendezvousMessage::new();
                                msg_out.set_punch_hole_response(PunchHoleResponse {
                                    pk: vec![] as Vec<u8>,
                                    failure: protobuf::ProtobufEnumOrUnknown::from(
                                        punch_hole_response::Failure::OFFLINE,
                                    ),
                                    relay_server: "39.107.33.253:21116".to_string(),
                                    union: std::option::Option::Some(
                                        punch_hole_response::Union::is_local(false),
                                    ),
                                    ..Default::default()
                                });
                                stream.send(&msg_out).await;
                                info!("{}", "640 对方不在线");
                                break;
                            }

                            let client = client?;
                            let mut msg_out = RendezvousMessage::new();
                            msg_out.set_punch_hole_response(PunchHoleResponse {
                                socket_addr: AddrMangle::encode(addr),
                                pk: vec![] as Vec<u8>,
                                relay_server: "39.107.33.253:21116".to_string(),
                                union: std::option::Option::Some(
                                    punch_hole_response::Union::is_local(false),
                                ),
                                ..Default::default()
                            });
                            stream.send(&msg_out).await?;
                            //记录两人ip匹配关系, 给lock加作用域

                            sender
                                .send(Event::First(remote_desk_id, client.local_addr))
                                .await;
                        }
                        Some(rendezvous_message::Union::request_relay(ph)) => {
                            allow_info!(format!(
                                "主动方 671 生成的uuid {:?}, 对方rustdeskId{:?},{:?}",
                                &ph.uuid, &ph.id, &addr
                            ));
                            let mut msg_out = RendezvousMessage::new();
                            msg_out.set_relay_response(RelayResponse {
                                socket_addr: vec![],
                                uuid: ph.uuid.clone(),
                                relay_server: "".to_string(),
                                refuse_reason: "".to_string(),
                                union: None,
                                unknown_fields: Default::default(),
                                cached_size: Default::default(),
                            });
                            stream.send(&msg_out).await?;

                            let mut s = state.lock().await;
                            s.kv16.insert(ph.id, addr);
                            s.receivers_17.insert(addr, rx.clone());
                            s.status.insert(addr, 1);
                            drop(s);
                            info!("{}", "682 finish");
                            break;
                        }

                        _ => {
                            allow_info!(format!("tcp21116_read_rendezvous_message {:?}", &msg_in));
                        }
                    }
                } else {
                    allow_info!(format!("tcp 21116 not match {:?}", &bytes));
                }
            } else {
                info!("699999 21116连接超时");
                break;
            }
        }
    }

    sleep(2 as f32).await;
    info!("701{}", "xxxxxxxxxxxxx");

    let (tx1, mut rx1) = unbounded::<Vec<u8>>();

    //给lock加作用域
    let mut re = None;
    {
        let mut s = state.lock().await;

        let mut key = "";
        for (k, v) in s.kv16.iter() {
            if v == &addr {
                key = k
            }
        }
        info!("21116 key ---------{}", key);
        info!("21116 kv17 ---------{:?}", s.kv17);
        let host = s.kv17.get(key).context("not found")?;

        if let Some(r) = s.receivers_16.get(host) {
            re = Some(r.clone());
            drop(s);
        }
    }
    if re.is_some() {
        println!("{}", "HHHHHHHHHHHHHH  message from 21117");
        tokio::spawn(fx2(tx1.clone(), re.unwrap()));
    }

    loop {
        select! {
        Ok(bytes) = rx1.recv() => {

              if let Ok(msg_in) = Message::parse_from_bytes(&bytes){
                 match msg_in.union {
                     //完成
                     Some(message::Union::hash(hash)) => {
                       allow_info!(format!("21119 Receiver hash {:?}", &hash));
                       let mut msg = Message::new();
                       msg.set_hash(hash);

                       stream.send_raw(msg.write_to_bytes().unwrap()).await;
                    }
                    //21116 recv
                    Some(message::Union::cmd_action(hash)) => {
                        println!("7644444444444{:?}",&hash);
                       let mut msg = Message::new();
                       msg.set_cmd_response(CMDResponse{
                              cmd:hash.cmd,
                            ..Default::default()
                            });

                       stream.send_raw(msg.write_to_bytes().unwrap()).await;
                    }
                     Some(message::Union::test_delay(hash)) => {
                       allow_info!(format!("21119 Receiver test_delay {:?}", &hash));
                       let mut msg = Message::new();
                       msg.set_test_delay(hash);
                       stream.send_raw(msg.write_to_bytes().unwrap()).await;
                        }
                     Some(message::Union::video_frame(hash)) => {

                                   let mut msg = Message::new();
                                   msg.set_video_frame(hash);
                                   stream.send_raw(msg.write_to_bytes().unwrap()).await;
                        }
                    Some(message::Union::login_response(hash)) => {

                                   let mut msg = Message::new();
                                   msg.set_login_response(hash);
                                   stream.send_raw(msg.write_to_bytes().unwrap()).await;
                               }

                       Some(message::Union::cursor_data(cd)) => {

                           let mut msg = Message::new();
                           msg.set_cursor_data(cd);
                           stream.send_raw(msg.write_to_bytes().unwrap()).await;
                       }


                     Some(message::Union::cursor_id(id)) => {
                         allow_info!(format!("21118 Receiver cursor_id{:?}", &id));
                         let mut msg = Message::new();
                         msg.set_cursor_id(id);
                         stream.send_raw(msg.write_to_bytes().unwrap()).await;

                     }

                     Some(message::Union::cursor_position(cp)) => {

                         let mut msg = Message::new();
                         msg.set_cursor_position(cp);
                         stream.send_raw(msg.write_to_bytes().unwrap()).await;
                     }
                     //完成
                     Some(message::Union::clipboard(cb)) => {

                         let mut msg = Message::new();
                         msg.set_clipboard(cb);
                         stream.send_raw(msg.write_to_bytes().unwrap()).await;
                     }
                     //暂存
                     Some(message::Union::file_response(fr)) => {

                         let mut msg = Message::new();
                         msg.set_file_response(fr);
                         stream.send_raw(msg.write_to_bytes().unwrap()).await;
                     }
                     Some(message::Union::misc(misc)) => {

                         let mut msg = Message::new();
                         msg.set_misc(misc);
                         stream.send_raw(msg.write_to_bytes().unwrap()).await;
                     }
                     Some(message::Union::audio_frame(frame)) => {

                         let mut msg = Message::new();
                         msg.set_audio_frame(frame);
                         stream.send_raw(msg.write_to_bytes().unwrap()).await;
                     }




                     _ => {
                         allow_info!(format!("tcp_active_21118  read_messages {:?}", &msg_in));
                     }

                     }
                 }
             }
         res =  stream.next_timeout(3000) =>  {
                if let Some(Ok(bytes)) =res{
             if let Ok(msg_in) = Message::parse_from_bytes(&bytes){
                 match msg_in.union{
                            //21116 send
                     Some(message::Union::cmd_action(hash)) => {
                     println!("858----------------{:?}",&hash);
                     let mut msg = Message::new();
                     msg.set_cmd_action(hash);
                     tx.send(msg.write_to_bytes().unwrap()).await;
                         }
                      Some(message::Union::signed_id(hash)) => {
                     allow_info!(format!("21116 passive signed_id {:?}", &hash));
                     let mut msg = Message::new();
                     msg.set_signed_id(hash);
                     tx.send(msg.write_to_bytes().unwrap()).await;
                         }
                     Some(message::Union::public_key(hash)) =>{
                           allow_info!(format!("21116 passive public_key {:?}", &hash));
                         let mut msg = Message::new();
                         msg.set_public_key(hash);
                         tx.send(msg.write_to_bytes().unwrap()).await;
                     }
                             //             //被动端转发延时请求给主动方
                     Some(message::Union::test_delay(hash)) => {
                         allow_info!(format!("21116 passive test_delay {:?}", &hash));
                         let mut msg = Message::new();
                         msg.set_test_delay(hash);
                         tx.send(msg.write_to_bytes().unwrap()).await;
                     }
                     Some(message::Union::video_frame(hash)) => {

                         let mut msg = Message::new();
                         msg.set_video_frame(hash);
                         tx.send(msg.write_to_bytes().unwrap()).await;
                     }
                         Some(message::Union::login_request(hash)) => {
                             allow_info!(format!("21116 passive login_request {:?}", &hash));
                             let mut msg = Message::new();
                             msg.set_login_request(hash);
                             tx.send(msg.write_to_bytes().unwrap()).await;
                         }
                         Some(message::Union::login_response(hash)) => {
                             allow_info!(format!("++++++++++++++++jjjjjj-1111  21116 login_response {:?}", &hash));
                             let mut msg = Message::new();
                             msg.set_login_response(hash);
                             tx.send(msg.write_to_bytes().unwrap()).await;
                         }

                         Some(message::Union::mouse_event(hash)) => {

                             let mut msg = Message::new();
                             msg.set_mouse_event(hash);
                             tx.send(msg.write_to_bytes().unwrap()).await;
                         }
                         Some(message::Union::audio_frame(hash)) => {

                             let mut msg = Message::new();
                             msg.set_audio_frame(hash);
                             tx.send(msg.write_to_bytes().unwrap()).await;
                         }
                         Some(message::Union::cursor_data(hash)) => {

                             let mut msg = Message::new();
                             msg.set_cursor_data(hash);
                             tx.send(msg.write_to_bytes().unwrap()).await;
                         }
                         Some(message::Union::cursor_position(hash)) => {

                             let mut msg = Message::new();
                             msg.set_cursor_position(hash);
                             tx.send(msg.write_to_bytes().unwrap()).await;
                         }
                         Some(message::Union::cursor_id(hash)) => {
                         allow_info!(format!("21116  cursor_id {:?}", &hash));
                         // tx.send(hash.write_to_bytes().unwrap()).await;
                         let mut msg = Message::new();
                         msg.set_cursor_id(hash);
                         tx.send(msg.write_to_bytes().unwrap()).await;

                         }
                         Some(message::Union::key_event(hash)) => {

                             let mut msg = Message::new();
                             msg.set_key_event(hash);
                             tx.send(msg.write_to_bytes().unwrap()).await;
                         }
                         Some(message::Union::clipboard(hash)) => {

                             let mut msg = Message::new();
                             msg.set_clipboard(hash);
                             tx.send(msg.write_to_bytes().unwrap()).await;
                         }
                         Some(message::Union::file_action(hash)) => {

                             let mut msg = Message::new();
                             msg.set_file_action(hash);
                             tx.send(msg.write_to_bytes().unwrap()).await;
                         }
                         Some(message::Union::file_response(hash)) => {

                             let mut msg = Message::new();
                             msg.set_file_response(hash);
                             tx.send(msg.write_to_bytes().unwrap()).await;
                         }
                         Some(message::Union::misc(hash)) => {

                             let mut msg = Message::new();
                             msg.set_misc(hash);
                             tx.send(msg.write_to_bytes().unwrap()).await;
                         }
                     //完成
                      _ => {
                     allow_info!(format!("tcp_21116  read_messages {:?}", &msg_in));
                 }
                    }
             }
                }else {
                info!("tcp 21116连接超时");
                break
            }
         }
         _ = Timer::after(Duration::from_secs(5)) =>{
                info!(" ticker 21116连接超时");
                break;
            }


         }
    }
    info!("drop stream  21116");
    drop(stream);

    Ok(())
}

async fn fx1(tx1: Sender<Vec<u8>>, r: Receiver<Vec<u8>>) {
    while let (Ok(t)) = r.recv().await {
        tx1.send(t).await;
    }
}

async fn fx2(tx1: Sender<Vec<u8>>, r: Receiver<Vec<u8>>) {
    while let (Ok(t)) = r.recv().await {
        tx1.send(t).await;
    }
}

async fn udp_21116(receiver: Receiver<Event>) -> Result<()> {
    let mut socket1 = UdpSocket::bind(to_socket_addr("0.0.0.0:21116").unwrap()).await?;
    let mut r = Arc::new(socket1);
    let mut s = r.clone();
    let mut socket = UdpFramed::new(r, LengthDelimitedCodec::new());
    let mut socket1 = UdpFramed::new(s, LengthDelimitedCodec::new());

    let receiver1 = receiver.clone();
    tokio::spawn(async move {
        while let Ok(eve) = receiver1.recv().await {
            match eve {
                //对端id,本地ip
                Event::First(id, a) => {
                    allow_info!(format!("{}", "first"));
                    udp_send_fetch_local_addr(&mut socket1, "39.107.33.253:21117".to_string(), a)
                        .await;

                    let ip_map = IpMap.clone();
                    //获取对端ip并更新
                    let result = ip_map.get(&id).unwrap();

                    let remote_ip = result.local_addr.ip().to_string();
                    let local_ip = a.ip().to_string();
                    let ip_map = IdMap.clone();
                    //更新自己的ip_map
                    ip_map.insert(local_ip,client{
                        timestamp: get_time() as u64,
                        local_addr: a.clone(),
                        peer_ip: remote_ip,
                        uuid: "".to_string()
                    });



                }
                Event::Second(id, b) => {
                    allow_info!(format!("{}", "second"));
                    let uuid = Uuid::new_v4().to_string();

                    udp_send_request_relay(
                        &mut socket1,
                        id,
                        uuid,
                        "39.107.33.253:21117".to_string(),
                        b,
                    )
                    .await;
                }
                Event::UnNone => {}
            }
        };
    }
    );

    loop {
        if let Some(Ok((bytes, addr))) = socket.next().await {
            handle_udp(&mut socket, bytes, addr, receiver.clone()).await;
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
    socket: &mut UdpFramed<LengthDelimitedCodec, Arc<UdpSocket>>,
    relay_server: String,
    addr: std::net::SocketAddr,
) -> Result<()> {
    let mut msg = RendezvousMessage::new();
    let addr1 = to_socket_addr("39.107.33.253:21117").unwrap();

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

//给被动方生成一个uuid
async fn udp_send_request_relay(
    socket: &mut UdpFramed<LengthDelimitedCodec, Arc<UdpSocket>>,
    id: String,
    uuid: String,
    relay_server: String,
    addr: std::net::SocketAddr,
) -> Result<()> {
    let mut msg = RendezvousMessage::new();

    msg.set_request_relay(RequestRelay {
        id: id,
        uuid: uuid,
        relay_server,
        ..Default::default()
    });
    //通过udp回复客户端打洞请求,提供一个中继cdn地址

    socket
        .send((Bytes::from(msg.write_to_bytes().unwrap()), addr))
        .await?;
    Ok(())
}

//建立对应关系表
async fn handle_udp(
    socket: &mut UdpFramed<LengthDelimitedCodec, Arc<UdpSocket>>,
    bytes: BytesMut,
    addr: std::net::SocketAddr,
    receiver: Receiver<Event>,
) {
    if let Ok(msg_in) = RendezvousMessage::parse_from_bytes(&bytes) {
        match msg_in.union {
            //不停的register_peer保持心跳,检测心跳告诉对方不在线
            Some(rendezvous_message::Union::register_peer(rp)) => {
                allow_info!(format!("register_peer {:?}", &addr));
                let mut msg = RendezvousMessage::new();
                let mut id_map = IdMap.clone();

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
                    rp.id.clone(),
                    client {
                        timestamp: get_time() as u64,
                        local_addr: addr,
                        peer_ip: "".to_string(),
                        uuid: "".to_string(),
                    },
                );
                //xp 注册ip和id
                let mut ip_map = IpMap.clone();
                let ip = addr.ip().to_string();
                let opt = ip_map.get(&ip);
                match opt {
                    None => {
                        ip_map.insert(
                            ip.clone(),
                            client {
                                timestamp: get_time() as u64,
                                local_addr: addr,
                                peer_ip: "".to_string(),
                                uuid: rp.id.clone(),
                            },
                        );
                    }
                    Some(ref e) => {
                        //取出存在peer_ip
                        let peer_ip = &e.peer_ip;
                        ip_map.insert(
                            ip,
                            client {
                                timestamp: get_time() as u64,
                                local_addr: addr,
                                peer_ip: String::from(peer_ip),
                                uuid: rp.id,
                            },
                        );
                    }
                }

                drop(id_map);
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

//自己的ip和对端ip配对
async fn proxy() -> ResultType<()> {
    let listener = TcpListener::bind("127.0.0.1:13389").await?;
    let (tx, _) = broadcast::channel(10);

    println!("chat server is ready");
    loop {
        let (mut socket, addr) = listener.accept().await?;
        println!("clint with addr {} is connected", addr);
        let tx = tx.clone();
        let mut rx = tx.subscribe();
        let ip = addr.ip().to_string();

        let mut stream = Framed::new(socket, BytesCodec::new());
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    results =  stream.next() => {
                     if let Some(Ok(bytes)) = results{
                             println!(" xxx {:?}",bytes);
                               tx.send((Bytes::from(bytes), ip.clone())).unwrap();
                        }else {
                            println!("{}",333);
                            break;
                        }
                    }
                    results = rx.recv() => {
                        let (message, other_ip) = results.unwrap();

                        let ip_map =  IpMap.clone();
                        let opt = ip_map.get(&ip);
                          match opt {
                                None => {
                                }
                                Some( ref e) => {
                                    //取出存在peer_ip
                                    let peer_ip = &e.peer_ip;
                                    if other_ip==String::from(peer_ip){
                                    stream.send(message).await;
                                }


                                }
                }

                    }
                }
            }
        });
    }
    Ok(())
}
