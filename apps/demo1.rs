


use std::io;
use hbb_common::tokio::select;
use hbb_common::{tokio, to_socket_addr, ResultType};
use hbb_common::udp::FramedSocket;


#[tokio::main]
async fn main() -> ResultType<()> {


    let mut socket = FramedSocket::new(to_socket_addr("0.0.0.0:0").unwrap()).await?;

    socket.send_raw("haha".as_bytes(),to_socket_addr("127.0.0.1:9000").unwrap()).await?;
    loop {
        select! {
            _ = socket.next() => {
           println!("4444");
            },
        }
    }

    Ok(())
}