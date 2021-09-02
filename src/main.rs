use crate::tokio::time::interval;
use hbb_common::message_proto::{Message, message};
use hbb_common::tokio::sync::mpsc;
use hbb_common::{
    bytes::BytesMut,
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

use std::collections::HashMap;
use tracing_subscriber;

#[derive(Debug, Clone)]
struct client {
    //心跳
    timestamp: u64,
    local_addr: std::net::SocketAddr,
    peer_addr: Option<std::net::SocketAddr>,
}

//默认情况下，hbbs 侦听 21115(tcp) 和 21116(tcp/udp)，hbbr 侦听 21117(tcp)。请务必在防火墙中打开这些端口
//上线离线问题, id,ip

#[tokio::main(basic_scheduler)]
async fn main() {
    tracing_subscriber::fmt::init();

    //udp 广播 socket
    let mut socket = FramedSocket::new("0.0.0.0:21116").await.unwrap();
    // 主动方连21116, 建立tcp 开启远程控制,中继回复被控方21117 建立远程控制
    let mut listener_active = new_listener("0.0.0.0:21116", false).await.unwrap();
    //
    let mut listener_passive = new_listener("0.0.0.0:21117", false).await.unwrap();
    let mut listener_passive1 = new_listener("0.0.0.0:21118", false).await.unwrap();

    //两个quic raw_data channel
    let (tx_from_active, mut rx_from_active) = mpsc::unbounded_channel::<Vec<u8>>();
    let (tx_from_passive, mut rx_from_passive) = mpsc::unbounded_channel::<Vec<u8>>();


    //把所有的远程rust_deskId存起来 //arc//RwLock
    let mut id_map = std::collections::HashMap::new();
    // let relay_server = std::env::var("IP").unwrap();

    // let mut saved_stream = None;
    const TIMER_OUT: Duration = Duration::from_secs(1);
    let mut timer = interval(TIMER_OUT);

    loop {
        tokio::select! {


        Some(Ok((bytes, addr))) = socket.next() => {
            handle_udp(&mut socket, bytes, addr, &mut id_map).await;
        }
         Ok((stream, addr))  = listener_passive1.accept() =>{
                 let mut stream = FramedStream::from(stream);
                if let Some(Ok(bytes)) = stream.next_timeout(3000).await{
                    if let Ok(msg_in) = Message::parse_from_bytes(&bytes){
                        match msg_in.union {
                         Some(message::Union::hash(hash)) => {
                            println!("8888888{:?}",&hash);

                        }
                        _ => {
                            println!("99999");
                        }
                    }
                    }

                }
            }

           Ok((stream, addr))  = listener_passive.accept() => {
                 let mut stream = FramedStream::from(stream);
                 if let Some(Ok(bytes)) = stream.next_timeout(3000).await {

                    if  let Ok(msg_in) = RendezvousMessage::parse_from_bytes(&bytes)  {
                         match msg_in.union {
                            Some(rendezvous_message::Union::local_addr(ph)) =>{
                                //第二步给对方发udp request_relay
                            let  remote_desk_id = ph.id;
                                println!("111111111{}",&remote_desk_id);
                                if let Some(client) = id_map.get(&remote_desk_id) {
                                   udp_send_request_relay(&mut socket,"47.88.2.164:21117".to_string(),client.local_addr).await;
                                }

                            }
                            Some(rendezvous_message::Union::relay_response(_)) => {
                                info!(" 2222222 relay_response {:?}", addr);
                            let mut msg_out = RendezvousMessage::new();
                            //提供一个 relay_server地址
                            msg_out.set_relay_response(RelayResponse {
                                relay_server: "47.88.2.164:21117".to_string(),
                                ..Default::default()
                            });

                            stream.send(&msg_out).await;

                        }
                            _ => {
                            println!("7777");
                            }
															//回复 test_nat_response
                        }
                    }else{
                        println!("44444444");
                    }
                }


        }
        Ok((stream, addr)) = listener_active.accept() => {
            let mut stream = FramedStream::from(stream);
            if let Some(Ok(bytes)) = stream.next_timeout(3000).await {


                if let Ok(msg_in) = RendezvousMessage::parse_from_bytes(&bytes) {
                    match msg_in.union {
                            Some(rendezvous_message::Union::test_nat_request(ph)) =>{
                       info!("test_nat_response {:?}",&ph);

                      let mut msg_out = RendezvousMessage::new();
                       //暂完成
                       msg_out.set_test_nat_response(TestNatResponse{
                        port:21117,
                        cu: protobuf::MessageField::some(ConfigUpdate{
                            ..Default::default()
                        }),
                        ..Default::default()
                    });

                        // //tcp加密发送
                        stream.send(&msg_out).await;

                    }
                            Some(rendezvous_message::Union::punch_hole(ph)) =>{
                       dbg!("test_nat_response",ph);
                        //回复 test_nat_response

                    }
                            Some(rendezvous_message::Union::punch_hole_sent(ph)) =>{
                       dbg!("test_nat_response",ph);
                        //回复 test_nat_response

                    }
                            Some(rendezvous_message::Union::local_addr(ph)) =>{

                            //第二步给对方发udp request_relay
                            let  remote_desk_id = ph.id;
                                 println!("qqqqqqqqqq{}",&remote_desk_id);
                               if let Some(client) = id_map.get(&remote_desk_id) {
                                    udp_send_request_relay(&mut socket,"47.88.2.164:21117".to_string(),client.local_addr).await;

                                }

                        }

                        Some(rendezvous_message::Union::request_relay(ph)) =>{
                            info!("llllllllllll{:?}",&ph);
                             let  remote_desk_id = ph.id;



                           let mut msg_out = RendezvousMessage::new();
                           msg_out.set_relay_response(RelayResponse{
                                      ..Default::default()
                                });
                           stream.send(&msg_out).await;



                        }

                        Some(rendezvous_message::Union::punch_hole_request(ph)) => {

                                 let  remote_desk_id = ph.id;
                                //给对方发udp广播fetch_local_addr

                               if let Some(client) = id_map.get(&remote_desk_id) {
                                     println!("pppppppp{:#?},{}",&id_map,&remote_desk_id);
                                     udp_send_fetch_local_addr(&mut socket,"47.88.2.164:21117".to_string(),client.local_addr).await;

                                }

                                let mut msg_out = RendezvousMessage::new();
                               //中继服务返回被控端的nat_type,中继cdn_ip,转发对方的加密的ip地址,公钥
                                let addr = stream.get_ref().local_addr().unwrap_or(to_socket_addr("47.88.2.164:21117").unwrap());
                                info!("punch_hole_request xxxxxxx{:?}",&addr);
                              msg_out.set_punch_hole_response(PunchHoleResponse{
                                socket_addr: AddrMangle::encode(addr),
                                pk: vec![] as Vec<u8>,
                                relay_server: "47.88.2.164:21116".to_string(),
                                union: std::option::Option::Some(punch_hole_response::Union::is_local(false)),
                                 ..Default::default()
                            });
                               stream.send(&msg_out).await;

                        }

                        _ => {}
                    }
                }else if let Ok(msg_in) = Message::parse_from_bytes(&bytes) {
                        info!("cccccccccccccc listen_active data {:?}",&msg_in);
                          tx_from_active.send(bytes.to_vec());

                             println!("{}","+++++++++++++++++");
                    //    if let Some(data) = rx_from_passive.recv().await{
                    //        info!("ddddddddddddd rx_from_passive data {:?}",&msg_in);
                    //         stream.send_raw(data).await;
                    //     }
                }
            }
        }

             Some(data) = rx_from_passive.recv() => {
                 println!("eeeeeeeeeeeeeee 收到被动端消息 send to 21116{:?}",&data);
                 if let Ok((stream_a, _)) = listener_active.accept().await {
                    let mut stream_a = FramedStream::from(stream_a);
                    stream_a.next_timeout(3_000).await;
                     stream_a.set_raw();
                     loop {
                        tokio::select! {

                            res = stream_a.next() => {
                                if let Some(Ok(bytes)) = res {
                                     println!("nnnnnnnnnnnn{:?}",&bytes);
                                    stream_a.send_bytes(bytes.into()).await.ok();
                                } else {
                                    break;
                                }
                            },

                                 }
                        }
                }


            }
            Some(data) = rx_from_active.recv() => {
                  println!("rrrrrrrrrrrr 收到主动端发的ui信息发给被动端  send to 21118{:?}",&data);

                 if let Ok((stream_a, _)) = listener_passive1.accept().await {
                    let mut stream_a = FramedStream::from(stream_a);
                    stream_a.next_timeout(3_000).await;
                    stream_a.set_raw();
                     loop {
                        tokio::select! {

                            res = stream_a.next() => {
                                if let Some(Ok(bytes)) = res {
                                                    println!("mmmmmmmmm{:?}",&bytes);
                                    stream_a.send_bytes(bytes.into()).await.ok();
                                } else {
                                    break;
                                }
                            },

                        }
    }
                }

            }

               _ = timer.tick() => {
                // 遍历ip_map 找出超出超时没发心跳的，告诉当方它已离线
            // println!("ticker ip_map_list{:#?}",&id_map);
             }

             }
    }
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
) {
    println!("sssssssss{:?}", &addr);
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
    socket.send(&msg_out, addr).await.ok();
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
) {
    let mut msg_out = RendezvousMessage::new();

    msg_out.set_request_relay(RequestRelay {
        relay_server,
        ..Default::default()
    });
    //通过udp回复客户端打洞请求,提供一个中继cdn地址
    socket.send(&msg_out, addr).await.ok();
}

async fn handle_udp(
    socket: &mut FramedSocket,
    bytes: BytesMut,
    addr: std::net::SocketAddr,
    id_map: &mut std::collections::HashMap<String, client>,
) {
    if let Ok(msg_in) = RendezvousMessage::parse_from_bytes(&bytes) {
        match msg_in.union {
            //不停的register_peer保持心跳,检测心跳告诉对方不在线
            Some(rendezvous_message::Union::register_peer(rp)) => {
                info!("register_peer {:?}", &addr);
                let mut msg_out = RendezvousMessage::new();
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

async fn relay(
    stream: FramedStream,
    peer: FramedStream,
    socket: &mut FramedSocket,
    id_map: &mut std::collections::HashMap<String, client>,
) {
    let mut peer = peer;
    let mut stream = stream;
    peer.set_raw();
    stream.set_raw();
    loop {
        tokio::select! {
            Some(Ok((bytes, addr))) = socket.next() => {
                //处理udp转发
                handle_udp(socket, bytes, addr, id_map).await;
            }
            res = peer.next() => {
                if let Some(Ok(bytes)) = res {
                    stream.send_bytes(bytes.into()).await.ok();
                } else {
                    break;
                }
            },
            res = stream.next() => {
                if let Some(Ok(bytes)) = res {
                    peer.send_bytes(bytes.into()).await.ok();
                } else {
                    break;
                }
            },
        }
    }
}
