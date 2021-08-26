use hbb_common::{
    bytes::BytesMut,
    protobuf::Message as _,
    rendezvous_proto::*,
    tcp::{new_listener, FramedStream},
    tokio,
    udp::FramedSocket,
};

//默认情况下，hbbs 侦听 21115(tcp) 和 21116(tcp/udp)，hbbr 侦听 21117(tcp)。请务必在防火墙中打开这些端口

#[tokio::main(basic_scheduler)]
async fn main() {
    //udp 广播 socket
    let mut socket = FramedSocket::new("0.0.0.0:21116").await.unwrap();
    //tcp listen
    let mut listener = new_listener("0.0.0.0:21116", false).await.unwrap();

    let mut rlistener = new_listener("0.0.0.0:21117", false).await.unwrap();
    //把所有的远程rust_deskId存起来
    let mut id_map = std::collections::HashMap::new();
    // let relay_server = std::env::var("IP").unwrap();
    let relay_server="211.94.141.2".to_string();
    let mut saved_stream = None;
    loop {
        tokio::select! {
            Some(Ok((bytes, addr))) = socket.next() => {
                //把收到的udp发出去
                handle_udp(&mut socket, bytes, addr, &mut id_map).await;
            }
            Ok((stream, addr)) = listener.accept() => {
                let mut stream = FramedStream::from(stream);
                if let Some(Ok(bytes)) = stream.next_timeout(3000).await {
                    println!("收到的内容{}",String::from_utf8_lossy(bytes.as_ref()));
                    if let Ok(msg_in) = RendezvousMessage::parse_from_bytes(&bytes) {
                        match msg_in.union {
                            Some(rendezvous_message::Union::test_nat_request(ph)) =>{
                               dbg!("test_nat_response",ph);
                                //回复 test_nat_response

                            }
                            Some(rendezvous_message::Union::test_nat_response(ph)) =>{
                               dbg!("test_nat_response",ph);
                                //回复 test_nat_response

                            }


                             Some(rendezvous_message::Union::punch_hole_request(ph)) =>{
                               dbg!("test_nat_response",ph);
                                //回复 test_nat_response

                            }
                             Some(rendezvous_message::Union::punch_hole(ph)) =>{
                               dbg!("test_nat_response",ph);
                                //回复 test_nat_response

                            }
                            Some(rendezvous_message::Union::punch_hole_sent(ph)) =>{
                               dbg!("test_nat_response",ph);
                                //回复 test_nat_response

                            }
                            Some(rendezvous_message::Union::punch_hole_response(ph)) =>{
                               dbg!("test_nat_response",ph);
                                //回复 test_nat_response

                            }

                             Some(rendezvous_message::Union::local_addr(ph)) =>{
                               dbg!("test_nat_response",ph);
                                //回复 test_nat_response

                            }
                            Some(rendezvous_message::Union::request_relay(ph)) =>{
                               dbg!("test_nat_response",ph);
                                //回复 test_nat_response

                            }

                            Some(rendezvous_message::Union::punch_hole_request(ph)) => {
                                println!("punch_hole_request {:?}", addr);
                                //如果在idMap里找到 rust_deskId ,ip 和收到的匹配,发动打洞回复，比给他提供一个 relay_server地址
                                if let Some(addr) = id_map.get(&ph.id) {
                                    // let mut msg_out = RendezvousMessage::new();
                                    // msg_out.set_request_relay(RequestRelay {
                                    //     relay_server: relay_server.clone(),
                                    //     ..Default::default()
                                    // });
                                    // //通过udp回复客户端打洞请求,提供一个中继cdn地址
                                    // socket.send(&msg_out, addr.clone()).await.ok();
                                    udp_send_request_relay(socket,addr).await
                                    saved_stream = Some(stream);
                                }
                            }
                            Some(rendezvous_message::Union::relay_response(_)) => {
                                println!("relay_response {:?}", addr);
                                let mut msg_out = RendezvousMessage::new();
                                //提供一个 relay_server地址
                                msg_out.set_relay_response(RelayResponse {
                                    relay_server: relay_server.clone(),
                                    ..Default::default()
                                });
                                if let Some(mut stream) = saved_stream.take() {
                                    //通过tcp回复客户端打洞请求,回复一个中继cdn地址
                                    //复制两份stream,给中继服务，中继服务发回去
                                    stream.send(&msg_out).await.ok();
                                    if let Ok((stream_a, _)) = rlistener.accept().await {
                                        let mut stream_a = FramedStream::from(stream_a);
                                        //延时3s
                                        stream_a.next_timeout(3_000).await;
                                        if let Ok((stream_b, _)) = rlistener.accept().await {
                                            let mut stream_b = FramedStream::from(stream_b);
                                            //延时3s
                                            stream_b.next_timeout(3_000).await;
                                            relay(stream_a, stream_b, &mut socket, &mut id_map).await;
                                        }
                                    }
                                }
                            }
                            _ => {
                                  println!("不匹配的指令");
                            }
                        }
                    }else{
                        println!("{}","不符合规则的数据");
                    }
                }
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
    let mut msg_out = FetchLocalAddr::new();
    msg_out.set_fetch_local_addr(msg_out);
    socket.send(&msg_out, addr).await.ok();
}

async fn udp_send_fetch_local_addr(
    socket: &mut FramedSocket,
    bytes: BytesMut,
    addr: std::net::SocketAddr,
    id_map: &mut std::collections::HashMap<String, std::net::SocketAddr>,
) {
    let mut msg_out = FetchLocalAddr::new();
    msg_out.set_fetch_local_addr(msg_out);
    socket.send(&msg_out, addr).await.ok();
}

async fn udp_send_punch_hole(
    socket: &mut FramedSocket,
    bytes: BytesMut,
    addr: std::net::SocketAddr,
    id_map: &mut std::collections::HashMap<String, std::net::SocketAddr>,
) {
    let mut msg_out = FetchLocalAddr::new();
    msg_out.set_fetch_local_addr(msg_out);
    socket.send(&msg_out, addr).await.ok();
}


async fn udp_send_configure_update(
    socket: &mut FramedSocket,
    bytes: BytesMut,
    addr: std::net::SocketAddr,
    id_map: &mut std::collections::HashMap<String, std::net::SocketAddr>,
) {
    let mut msg_out = FetchLocalAddr::new();
    msg_out.set_fetch_local_addr(msg_out);
    socket.send(&msg_out, addr).await.ok();
}


async fn udp_send_request_relay(
    socket: &mut FramedSocket,
    addr: std::net::SocketAddr,

) {
    let mut msg_out = RendezvousMessage::new();
    msg_out.set_request_relay(RequestRelay {
        relay_server: relay_server.clone(),
        ..Default::default()
    });
    //通过udp回复客户端打洞请求,提供一个中继cdn地址
    socket.send(&msg_out, addr.clone()).await.ok();
}



async fn handle_udp(
    socket: &mut FramedSocket,
    bytes: BytesMut,
    addr: std::net::SocketAddr,
    id_map: &mut std::collections::HashMap<String, std::net::SocketAddr>,
) {
    if let Ok(msg_in) = RendezvousMessage::parse_from_bytes(&bytes) {
        match msg_in.union {
            Some(rendezvous_message::Union::register_peer(rp)) => {
                println!("register_peer {:?}", addr);
                id_map.insert(rp.id, addr);
                let mut msg_out = RendezvousMessage::new();
                msg_out.set_register_peer_response(RegisterPeerResponse::new());
                socket.send(&msg_out, addr).await.ok();
            }
            Some(rendezvous_message::Union::register_pk(_)) => {
                println!("register_pk {:?}", addr);
                let mut msg_out = RendezvousMessage::new();
                msg_out.set_register_pk_response(RegisterPkResponse {
                    result: register_pk_response::Result::OK.into(),
                    ..Default::default()
                });
                socket.send(&msg_out, addr).await.ok();
            }
            Some(rendezvous_message::Union::configure_update(rp)) => {
                println!("register_peer {:?}", addr);
                id_map.insert(rp.id, addr);
                let mut msg_out = ConfigUpdate::new();
                msg_out.set_configure_update(msg_out);
                socket.send(&msg_out, addr).await.ok();
            }
            Some(rendezvous_message::Union::software_update(rp)) => {
                println!("register_peer {:?}", addr);
                id_map.insert(rp.id, addr);
                let mut msg_out = SoftwareUpdate::new();
                msg_out.set_software_update(msg_out);
                socket.send(&msg_out, addr).await.ok();
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
    id_map: &mut std::collections::HashMap<String, std::net::SocketAddr>,
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

    use hbb_common::{ResultType, to_socket_addr};
    use hbb_common::config::{Config, RENDEZVOUS_TIMEOUT};
    use hbb_common::anyhow::Context;
    use std::net::{SocketAddr, ToSocketAddrs};

    // pub fn to_socket_addr(host: &str) -> ResultType<SocketAddr> {
    //     let addrs: Vec<SocketAddr> = host.to_socket_addrs()?.collect();
    //     if addrs.is_empty() {
    //         bail!("Failed to solve {}", host);
    //     }
    //     Ok(addrs[0])
    // }

    #[tokio::test]
    async fn udp_send_raw_test() -> ResultType<()> {
        let mut socket = FramedSocket::new(Config::get_any_listen_addr()).await?;
        socket.send_raw("ahahha".as_ref(), to_socket_addr("127.0.0.1:15000").unwrap()).await?;

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
        socket.send_raw("ahahha".as_ref(), to_socket_addr("127.0.0.1:15000").unwrap()).await?;

        Ok(())
    }

    #[tokio::test]
    async fn udp_rcv_rendezvous_message() -> ResultType<()> {
        let mut socket = FramedSocket::new(Config::get_any_listen_addr()).await?;
        socket.send_raw("ahahha".as_ref(), to_socket_addr("127.0.0.1:15000").unwrap()).await?;

        Ok(())
    }





    #[tokio::test]
    async fn tcp_send_raw()-> ResultType<()> {
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
        println!("{}",&my_addr );
        socket.send_raw(Vec::from("hahhahhhhhhhhhh".as_bytes())).await?;
        Ok(())
    }


    #[tokio::test]
    async fn tcp_rcv_raw()-> ResultType<()> {
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
        println!("{}",&my_addr );
        socket.send_raw(Vec::from("hahhahhhhhhhhhh".as_bytes())).await?;
        Ok(())
    }

    #[tokio::test]
    async fn tcp_rcv_encode()-> ResultType<()> {

        Ok(())
    }

    #[tokio::test]
    async fn tcp_rcv_decode()-> ResultType<()> {

        Ok(())
    }
    #[tokio::test]
    async fn tcp_send_Message()-> ResultType<()> {
        let server_addr = to_socket_addr("176.122.144.113:21116").unwrap();
        let client_addr = to_socket_addr("0.0.0.0:0").unwrap();
        let mut stream = FramedStream::new(
            server_addr,
            client_addr,
            RENDEZVOUS_TIMEOUT,
        )
            .await?;
        let mut msg_out = RendezvousMessage::new();
        msg_out.set_test_nat_request(TestNatRequest {
            serial:1,
            ..Default::default()
        });
        stream.send(&msg_out).await?;
       if let Some(Ok(bytes)) = stream.next_timeout(3000).await{
           dbg!(bytes);
       }

        Ok(())
    }


    #[tokio::test]
    async fn tcp_rcv_Message()-> ResultType<()> {

        Ok(())
    }

}
