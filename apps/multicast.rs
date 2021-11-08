use std::error::Error;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};


use hbb_common::ResultType;
use hbb_common::bytes::BytesMut;

use hbb_common::futures_util::StreamExt;
use hbb_common::tokio::net::UdpSocket;
use hbb_common::tokio_util::udp::UdpFramed;

use hbb_common::tokio_util::codec::{BytesCodec, Framed};

use hbb_common::tokio;
use hbb_common::futures::SinkExt;
use hbb_common::bytes::Bytes;
use hbb_common::message_proto::{Message, message};
use hbb_common::protobuf::Message as _;


const DEFAULT_USERNAME: &str = "Anonymous";
const DEFAULT_PORT: &str = "5050";
const DEFAULT_MULTICAST: &str = "239.1.1.50";
const IP_ALL: [u8; 4] = [0, 0, 0, 0];

/// Bind socket to multicast address with IP_MULTICAST_LOOP and SO_REUSEADDR Enabled
fn bind_multicast(
    addr: &SocketAddrV4,
    multi_addr: &SocketAddrV4,
) -> ResultType<std::net::UdpSocket> {
    use socket2::{Domain, Protocol, Socket, Type};

    assert!(multi_addr.ip().is_multicast(), "Must be multcast address");

    let socket = Socket::new(Domain::ipv4(), Type::dgram(), Some(Protocol::udp()))?;

    socket.set_reuse_address(true)?;
    socket.bind(&socket2::SockAddr::from(*addr))?;
    socket.set_multicast_loop_v4(true)?;
    socket.join_multicast_v4(multi_addr.ip(), addr.ip())?;

    Ok(socket.into_udp_socket())
}

#[tokio::main]
async fn main() -> ResultType<()> {
    let username = "Anonymous".to_string();

    let port = 5050;

    let addr = SocketAddrV4::new(IP_ALL.into(), 5050);

    let multi_addr = SocketAddrV4::new("239.1.1.50".parse::<Ipv4Addr>().expect("Invalid IP"), port);

    println!("Starting server on: {}", addr);
    println!("Multicast address: {}\n", multi_addr);

    let std_socket = bind_multicast(&addr, &multi_addr).expect("Failed to bind multicast socket");

    let socket1 = UdpSocket::from_std(std_socket).unwrap();
    let mut socket = UdpFramed::new(socket1, BytesCodec::new());

    loop {
        if let Some(Ok((bytes, addr))) = socket.next().await {
            handle_udp1(&mut socket, bytes, addr).await;
        }
    }


}







async fn handle_udp(
    socket: &mut UdpFramed<BytesCodec, UdpSocket>,
    bytes: BytesMut,
    addr: std::net::SocketAddr,
) {
    println!("addr {},{:?},",addr,bytes );
    if let Ok(msg_in) = Message::parse_from_bytes(&bytes) {
        match msg_in.union {
            None => {}
            Some(message::Union::cmd_action(hash)) => {

            }
            Some(message::Union::login_request(hash)) =>{

            }
            Some(message::Union::test_delay(hash)) =>{

            }
            Some(message::Union::video_frame(hash)) =>{

            }
            Some(message::Union::login_response(hash)) =>{

            }
            Some(message::Union::cursor_data(cd)) =>{

            }
            Some(message::Union::cursor_id(id)) =>{

            }
            Some(message::Union::cursor_position(cp)) =>{

            }
            Some(message::Union::clipboard(cb)) =>{

            }
            Some(message::Union::file_response(fr)) =>{

            }

            Some(message::Union::misc(misc)) =>{

            }
            Some(message::Union::audio_frame(frame)) => {

            }
            _ => {}
        }
    }
}
