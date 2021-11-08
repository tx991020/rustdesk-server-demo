#[cfg(test)]
mod tests {
    use hbb_common::tokio::select;

    use hbb_common::anyhow::Context;
    use hbb_common::config::{Config, RENDEZVOUS_TIMEOUT};
    use hbb_common::protobuf::{Message, ProtobufEnum};
    use hbb_common::rendezvous_proto::{
        LocalAddr, PunchHoleRequest, RegisterPeer, RegisterPk, RelayResponse, RendezvousMessage,
        RequestRelay, TestNatRequest,
    };
    use hbb_common::tcp::FramedStream;
    use hbb_common::udp::FramedSocket;
    use hbb_common::{to_socket_addr, AddrMangle, ResultType};
    use std::net::{SocketAddr, ToSocketAddrs};
    use uuid::Uuid;

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
        let srever_addr = to_socket_addr("47.88.2.164:21117").unwrap();
        let mut stream = FramedStream::new(
            srever_addr,
            Config::get_any_listen_addr(),
            RENDEZVOUS_TIMEOUT,
        )
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
        let srever_addr = to_socket_addr("47.88.2.164:21117").unwrap();
        let mut stream = FramedStream::new(
            srever_addr,
            Config::get_any_listen_addr(),
            RENDEZVOUS_TIMEOUT,
        )
        .await
        .with_context(|| "Failed to connect to rendezvous server")?;
        let mut msg_out = RendezvousMessage::new();
        let uuid = Uuid::new_v4().to_string();
        msg_out.set_request_relay(RequestRelay {
            id: "460351640".to_string(),
            uuid: uuid.clone(),
            relay_server: "47.88.2.164:21117".to_string(),
            secure: true,
            ..Default::default()
        });
        stream.send(&msg_out).await?;

        Ok(())
    }
    //主控第三步
    #[tokio::test]
    async fn tcp_set_request_relay_cdn() -> ResultType<()> {
        let srever_addr = to_socket_addr("47.88.2.164:21117").unwrap();
        let mut stream = FramedStream::new(
            srever_addr,
            Config::get_any_listen_addr(),
            RENDEZVOUS_TIMEOUT,
        )
        .await
        .with_context(|| "Failed to connect to rendezvous server")?;
        let mut msg_out = RendezvousMessage::new();
        let uuid = Uuid::new_v4().to_string();
        msg_out.set_request_relay(RequestRelay {
            id: "460351640".to_string(),
            uuid: uuid.clone(),
            relay_server: "47.88.2.164:21117".to_string(),
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
        let mut stream = FramedStream::new(
            srever_addr,
            Config::get_any_listen_addr(),
            RENDEZVOUS_TIMEOUT,
        )
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
        let mut stream = FramedStream::new(
            srever_addr,
            Config::get_any_listen_addr(),
            RENDEZVOUS_TIMEOUT,
        )
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
        let mut stream = FramedStream::new(
            srever_addr,
            Config::get_any_listen_addr(),
            RENDEZVOUS_TIMEOUT,
        )
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

fn main() {}
