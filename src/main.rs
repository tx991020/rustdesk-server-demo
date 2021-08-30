use crate::tokio::time::interval;
use hbb_common::message_proto::Message;
use hbb_common::tokio::sync::mpsc;
use hbb_common::{bytes::BytesMut, protobuf, protobuf::Message as _, rendezvous_proto::*, sleep, tcp::{new_listener, FramedStream}, to_socket_addr, tokio, udp::FramedSocket, AddrMangle, utils};
use std::ops::Deref;
use std::time::Duration;
use uuid::Uuid;
use std::net::SocketAddr;

#[macro_use]
extern crate tracing;

use tracing_subscriber;
use std::collections::HashMap;

#[derive(Debug,Clone)]
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
    let mut listener_a = new_listener("0.0.0.0:21116", false).await.unwrap();
    //
    let mut listener_b = new_listener("0.0.0.0:21117", false).await.unwrap();

    //两个quic raw_data channel
    let (tx_from_a, mut rx_from_b) = mpsc::unbounded_channel::<Vec<u8>>();
    let (tx_to_b, rx_to_b) = mpsc::unbounded_channel::<Vec<u8>>();

    //两个tcp channel
    let (tx_from_c, mut rx_from_c) = mpsc::unbounded_channel::<Vec<u8>>();
    let (tx_to_d, rx_to_d) = mpsc::unbounded_channel::<Vec<u8>>();

    //把所有的远程rust_deskId存起来 //arc//RwLock
    let mut id_map = std::collections::HashMap::new();
    // let relay_server = std::env::var("IP").unwrap();
    let relay_server = "176.122.144.113:21117".to_string();
    // let mut saved_stream = None;
    const TIMER_OUT: Duration = Duration::from_secs(1);
    let mut timer = interval(TIMER_OUT);

    loop {
        tokio::select! {

        Some(Ok((bytes, addr))) = socket.next() => {
            handle_udp(&mut socket, bytes, addr, &mut id_map).await;
        }
        Ok((stream, addr))  = listener_b.accept() => {

        }
        Ok((stream, addr)) = listener_a.accept() => {
            let mut stream = FramedStream::from(stream);
            if let Some(Ok(bytes)) = stream.next_timeout(3000).await {
                if let Ok(msg_in) = RendezvousMessage::parse_from_bytes(&bytes) {
                    match msg_in.union {
                            Some(rendezvous_message::Union::punch_hole(ph)) =>{
                       dbg!("test_nat_response",ph);
                        //回复 test_nat_response

                            }
                            Some(rendezvous_message::Union::test_nat_request(ph)) =>{
                       info!("test_nat_response zzzz",&ph);
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
                            Some(rendezvous_message::Union::punch_hole_sent(ph)) =>{
                       dbg!("test_nat_response",ph);
                        //回复 test_nat_response

                    }
                            Some(rendezvous_message::Union::local_addr(ph)) =>{
                           dbg!("test_nat_response",ph);
                            //回复 test_nat_response

                        }
                        Some(rendezvous_message::Union::request_relay(ph)) =>{
                             let  remote_desk_id = ph.id;
                                //第二步给对方发udp request_relay
                               if let Some(client) = id_map.get(&remote_desk_id) {
                                    udp_send_request_relay(&mut socket,"176.122.144.113".to_string()).await;
                                }

                           let mut msg_out = RendezvousMessage::new();
                           msg_out.set_relay_response(RelayResponse{
                                      ..Default::default()
                                })

                           stream.send(&msg_out).await;



                        }

                        Some(rendezvous_message::Union::punch_hole_request(ph)) => {
                               let  remote_desk_id = ph.id;
                                //第一步给对方发udp广播fetch_local_addr
                               if let Some(client) = id_map.get(&remote_desk_id) {
                                     udp_send_fetch_local_addr(socket,"176.122.144.113".to_string()).await;
                                }
                                let mut msg_out = RendezvousMessage::new();
                               //中继服务返回被控端的nat_type,中继cdn_ip,转发对方的加密的ip地址,公钥
                                let addr = stream.get_ref().local_addr().unwrap_or(to_socket_addr("47.88.2.164:21116").unwrap());
                              msg_out.set_punch_hole_response(PunchHoleResponse{
                                 socket_addr: AddrMangle::encode(addr),
                                pk: vec![] as Vec<u8>,
                                relay_server: "47.88.2.164:21117".to_string(),
                                union: std::option::Option::Some(punch_hole_response::Union::is_local(false)),
                                 ..Default::default()
                            });
                                stream.send(msg_out).await;



                        }
                        // Some(rendezvous_message::Union::relay_response(_)) => {
                        //         println!("relay_response qqqqqqqq {:?}", addr);
                        //     let mut msg_out = RendezvousMessage::new();
                        //     //提供一个 relay_server地址
                        //     msg_out.set_relay_response(RelayResponse {
                        //         relay_server: "47.88.2.164:21117".to_string(),,
                        //         ..Default::default()
                        //     });
                        //         stream.send(msg_out).await;
                        //     // if let Some(mut stream) = saved_stream.take() {
                        //     //     //通过tcp回复客户端打洞请求,回复一个中继cdn地址
                        //     //     //复制两份stream,给中继服务，中继服务发回去
                        //     //     stream.send(&msg_out).await.ok();
                        //     //     if let Ok((stream_a, _)) = listener_b.accept().await {
                        //     //         let mut stream_a = FramedStream::from(stream_a);
                        //     //         //延时3s
                        //     //         stream_a.next_timeout(3_000).await;
                        //     //         if let Ok((stream_b, _)) = listener_b.accept().await {
                        //     //             let mut stream_b = FramedStream::from(stream_b);
                        //     //             //延时3s
                        //     //             stream_b.next_timeout(3_000).await;
                        //     //             relay(stream_a, stream_b, &mut socket, &mut id_map).await;
                        //     //         }
                        //     //     }
                        //     // }
                        // }
                        _ => {}
                    }
                }
            }
        }
              _ = timer.tick() => {
                // 遍历ip_map 找出超出超时没发心跳的，告诉当方它已离线
            println!("ip_map_list{}","ticker");

             }
    }
    }

//
//

//
//                         // Some(rendezvous_message::Union::punch_hole_request(ph)) => {
//                         //     println!("punch_hole_request {:?}", addr);
//                         //     //如果在idMap里找到 rust_deskId ,ip 和收到的匹配,发动打洞回复，比给他提供一个 relay_server地址
//                         //     if let Some(addr) = id_map.get(&ph.id) {
//                         //         // let mut msg_out = RendezvousMessage::new();
//                         //         // msg_out.set_request_relay(RequestRelay {
//                         //         //     relay_server: relay_server.clone(),
//                         //         //     ..Default::default()
//                         //         // });
//                         //         // //通过udp回复客户端打洞请求,提供一个中继cdn地址
//                         //         // socket.send(&msg_out, addr.clone()).await.ok();
//                         //         udp_send_request_relay(&mut socket,addr.clone()).await;
//                         //         saved_stream = Some(stream);
//                         //     }
//                         // }




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
        relay_server:String,
    ) {
        let mut msg_out = RendezvousMessage::new();
        let addr = socket.get_ref().local_addr().unwrap_or(to_socket_addr("").unwrap());
        let vec1 = AddrMangle::encode(addr);
        msg_out.set_fetch_local_addr(FetchLocalAddr {
            socket_addr: vec1,
            relay_server,
            ..Default::default()
        });
        let mut msg_out = FetchLocalAddr::new();
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

    async fn udp_send_request_relay(socket: &mut FramedSocket, relay_server:String) {
        let mut msg_out = RendezvousMessage::new();
        let addr = socket.get_ref().local_addr().unwrap_or(to_socket_addr("").unwrap());
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
                    println!("register_peer {:?}", &addr);
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
                    id_map.insert(rp.id, client { timestamp: utils::now(), local_addr: addr, peer_addr: None });
                    socket.send(&msg_out, addr.clone()).await.ok();
                }
                //完成
                Some(rendezvous_message::Union::register_pk(rp)) => {
                    println!("register_pk {:?}", addr);
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

    #[cfg(test)]
    mod tests {
        use super::*;
        use tokio::select;

        use hbb_common::anyhow::Context;
        use hbb_common::config::{Config, RENDEZVOUS_TIMEOUT};
        use hbb_common::{to_socket_addr, ResultType};
        use std::net::{SocketAddr, ToSocketAddrs};
        use hbb_common::protobuf::ProtobufEnum;

        // pub fn to_socket_addr(host: &str) -> ResultType<SocketAddr> {
        //     let addrs: Vec<SocketAddr> = host.to_socket_addrs()?.collect();
        //     if addrs.is_empty() {
        //         bail!("Failed to solve {}", host);
        //     }
        //     Ok(addrs[0])
        // }

        #[tokio::test]
        async fn register_pk_test() -> ResultType<()> {
            let mut socket = FramedSocket::new(Config::get_any_listen_addr()).await?;
            let srever_addr = to_socket_addr("47.88.2.164:21116").unwrap();

            let mut msg_out = RendezvousMessage::new();
            //获取公钥
            let pk = Config::get_key_pair().1;
            let uuid = pk.clone();
            let id = Config::get_id();

            //把Library/Preferences/com.carriez.RustDesk/RustDesk.toml里 rustdesk_id, 本机uuid,公钥发给中继
            msg_out.set_register_pk(RegisterPk {
                id,
                uuid,
                pk,
                ..Default::default()
            });
            socket.send(&msg_out, srever_addr).await?;
            Ok(())
        }


        #[tokio::test]
        async fn register_peer_test() -> ResultType<()> {
            let mut socket = FramedSocket::new(Config::get_any_listen_addr()).await?;
            let srever_addr = to_socket_addr("47.88.2.164:21116").unwrap();

            let local_desk_id = Config::get_id();

            let serial = Config::get_serial();
            let mut msg_out = RendezvousMessage::new();
            let mut msg_out = RendezvousMessage::new();
            msg_out.set_register_peer(RegisterPeer {
                id: local_desk_id,
                serial,
                ..Default::default()
            });

            socket.send(&msg_out, srever_addr).await?;
            Ok(())
        }

        //主控第一步，打洞
        #[tokio::test]
        async fn tcp_punch_hole_request1() -> ResultType<()> {
            let srever_addr = to_socket_addr("47.88.2.164:21116").unwrap();
            let mut stream = FramedStream::new(srever_addr, Config::get_any_listen_addr(), RENDEZVOUS_TIMEOUT)
                .await
                .with_context(|| "Failed to connect to rendezvous server")?;
            let mut msg_out = RendezvousMessage::new();
            msg_out.set_punch_hole_request(PunchHoleRequest {
                //远程rust_desk_id
                id: "460351640".to_string(),
                //自己的nat_type
                ..Default::default()
            });
            // msg_out.set_punch_hole_request(PunchHoleRequest {
            //     //远程rust_desk_id
            //     id: remote_desk_id.to_owned(),
            //     //自己的nat_type
            //     nat_type: nat_type.into(),
            //     conn_type: conn_type.into(),
            //     ..Default::default()
            // });

            stream.send(&msg_out).await?;
            Ok(())
        }


        //主控第二步
        #[tokio::test]
        async fn tcp_set_request_relay_rendez() -> ResultType<()> {
            let srever_addr = to_socket_addr("47.88.2.164:21116").unwrap();
            let mut stream = FramedStream::new(srever_addr, Config::get_any_listen_addr(), RENDEZVOUS_TIMEOUT)
                .await
                .with_context(|| "Failed to connect to rendezvous server")?;
            let mut msg_out = RendezvousMessage::new();
            let uuid = Uuid::new_v4().to_string();
            msg_out.set_request_relay(RequestRelay {
                id: "460351640".to_string(),
                uuid: uuid.clone(),
                relay_server: "47.88.2.164:21116".to_string(),
                secure: true,
                ..Default::default()
            });
            stream.send(&msg_out).await?;

            Ok(())
        }

        //主控第三步
        #[tokio::test]
        async fn tcp_set_request_relay_cdn() -> ResultType<()> {
            let srever_addr = to_socket_addr("47.88.2.164:21116").unwrap();
            let mut stream = FramedStream::new(srever_addr, Config::get_any_listen_addr(), RENDEZVOUS_TIMEOUT)
                .await
                .with_context(|| "Failed to connect to rendezvous server")?;
            let mut msg_out = RendezvousMessage::new();
            let uuid = Uuid::new_v4().to_string();
            msg_out.set_request_relay(RequestRelay {
                id: "460351640".to_string(),
                uuid: uuid.clone(),
                relay_server: "47.88.2.164:21116".to_string(),
                secure: true,
                ..Default::default()
            });
            stream.send(&msg_out).await?;

            Ok(())
        }

        //被控第二步 中继回复
        #[tokio::test]
        async fn tcp_send_local_addr() -> ResultType<()> {
            let srever_addr = to_socket_addr("47.88.2.164:21116").unwrap();
            let mut stream = FramedStream::new(srever_addr, Config::get_any_listen_addr(), RENDEZVOUS_TIMEOUT)
                .await
                .with_context(|| "Failed to connect to rendezvous server")?;
            let mut msg_out = RendezvousMessage::new();
            let local_addr = stream.get_ref().local_addr()?;
            msg_out.set_local_addr(LocalAddr {
                id: Config::get_id(),
                //主控的公网地址
                // socket_addr: AddrMangle::encode(peer_addr),
                //被控内网地址
                local_addr: AddrMangle::encode(local_addr),
                //中继cdn地址
                relay_server: "47.88.2.164:21116".to_string(),
                ..Default::default()
            });
            let bytes = msg_out.write_to_bytes()?;
            stream.send_raw(bytes).await?;
            Ok(())
        }

        //被控第三步 create_relay
        #[tokio::test]
        async fn tcp_set_relay_response() -> ResultType<()> {
            let srever_addr = to_socket_addr("47.88.2.164:21116").unwrap();
            let mut stream = FramedStream::new(srever_addr, Config::get_any_listen_addr(), RENDEZVOUS_TIMEOUT)
                .await
                .with_context(|| "Failed to connect to rendezvous server")?;
            let vec = AddrMangle::encode(to_socket_addr("0.0.0.0:0").unwrap());
            let mut msg_out = RendezvousMessage::new();
            let mut rr = RelayResponse {
                socket_addr: vec,
                ..Default::default()
            };

            msg_out.set_relay_response(rr);
            stream.send(&msg_out).await?;
            Ok(())
        }

        //被控第四步 create_relay_connection_
        #[tokio::test]
        async fn tcp_set_request_relay() -> ResultType<()> {
            let srever_addr = to_socket_addr("47.88.2.164:21116").unwrap();
            let mut stream = FramedStream::new(srever_addr, Config::get_any_listen_addr(), RENDEZVOUS_TIMEOUT)
                .await
                .with_context(|| "Failed to connect to rendezvous server")?;
            let vec = AddrMangle::encode(to_socket_addr("0.0.0.0:0").unwrap());
            let mut msg_out = RendezvousMessage::new();
            msg_out.set_request_relay(RequestRelay {
                uuid,
                ..Default::default()
            });
            stream.send(&msg_out).await?;
            Ok(())
        }


        #[tokio::test]
        async fn udp_rcv_raw() -> ResultType<()> {
            let mut socket = FramedSocket::new(to_socket_addr("127.0.0.1:8000").unwrap()).await?;

            select! {
            Some(Ok((bytes, _))) = socket.next() => {
           println!("{:?}",bytes);
            },
        }

            Ok(())
        }

        #[tokio::test]
        async fn udp_send_rendezvous_message() -> ResultType<()> {
            let mut socket = FramedSocket::new(Config::get_any_listen_addr()).await?;
            socket
                .send_raw(
                    "ahahha".as_ref(),
                    to_socket_addr("127.0.0.1:15000").unwrap(),
                )
                .await?;

            Ok(())
        }

        #[tokio::test]
        async fn udp_rcv_rendezvous_message() -> ResultType<()> {

            let mut socket = FramedSocket::new(Config::get_any_listen_addr()).await?;
            socket
                .send_raw(
                    "ahahha".as_ref(),
                    to_socket_addr("127.0.0.1:15000").unwrap(),
                )
                .await?;

            Ok(())
        }

        #[tokio::test]
        async fn tcp_send_raw() -> ResultType<()> {
            // let any_addr = Config::get_any_listen_addr();
            //
            // let mut socket = FramedStream::new(to_socket_addr("216.128.140.17:21117").unwrap(), any_addr, RENDEZVOUS_TIMEOUT)
            //     .await
            //     .with_context(|| "Failed to connect to rendezvous server")?;
            // dbg!(socket);
            // //能查本机的内网ip

            let server_addr = to_socket_addr("198.18.0.1:55468").unwrap();
            let client_addr = to_socket_addr("0.0.0.0:0").unwrap();
            let mut socket = FramedStream::new(server_addr, client_addr, RENDEZVOUS_TIMEOUT)
                .await
                .with_context(|| "Failed to connect to rendezvous server")?;
            let my_addr = socket.get_ref().local_addr()?;
            println!("{}", &my_addr);
            socket
                .send_raw(Vec::from("hahhahhhhhhhhhh".as_bytes()))
                .await?;
            Ok(())
        }

        #[tokio::test]
        async fn tcp_rcv_raw() -> ResultType<()> {
            // let any_addr = Config::get_any_listen_addr();
            //
            // let mut socket = FramedStream::new(to_socket_addr("216.128.140.17:21117").unwrap(), any_addr, RENDEZVOUS_TIMEOUT)
            //     .await
            //     .with_context(|| "Failed to connect to rendezvous server")?;
            // dbg!(socket);
            // //能查本机的内网ip

            let server_addr = to_socket_addr("198.18.0.1:55468").unwrap();
            let client_addr = to_socket_addr("0.0.0.0:0").unwrap();
            let mut socket = FramedStream::new(server_addr, client_addr, RENDEZVOUS_TIMEOUT)
                .await
                .with_context(|| "Failed to connect to rendezvous server")?;
            let my_addr = socket.get_ref().local_addr()?;
            println!("{}", &my_addr);
            socket
                .send_raw(Vec::from("hahhahhhhhhhhhh".as_bytes()))
                .await?;
            Ok(())
        }

        #[tokio::test]
        async fn tcp_rcv_encode() -> ResultType<()> {
            Ok(())
        }

        #[tokio::test]
        async fn tcp_rcv_decode() -> ResultType<()> {
            Ok(())
        }

        #[tokio::test]
        async fn tcp_send_encode_Message() -> ResultType<()> {
            let server_addr = to_socket_addr("47.88.2.164:21116").unwrap();
            // let server_addr = to_socket_addr("176.122.144.113:21116").unwrap();
            let client_addr = to_socket_addr("0.0.0.0:0").unwrap();
            let mut stream = FramedStream::new(server_addr, client_addr, RENDEZVOUS_TIMEOUT).await?;
            let mut msg_out = RendezvousMessage::new();
            msg_out.set_test_nat_request(TestNatRequest {
                serial: 1,
                ..Default::default()
            });
            stream.send(&msg_out).await?;
            if let Some(Ok(bytes)) = stream.next_timeout(3000).await {
                dbg!(bytes);
            }

            Ok(())
        }

        #[tokio::test]
        async fn tcp_rcv_Message() -> ResultType<()> {
            Ok(())
        }
    }
}
