use hbb_common::tokio::net::UdpSocket;
use std::{io, net::SocketAddr, sync::Arc};

use hbb_common::futures::SinkExt;
use hbb_common::tokio;
use hbb_common::tokio::io::AsyncBufReadExt;
use hbb_common::tokio::sync::mpsc;

#[tokio::main]
async fn main() -> io::Result<()> {
    let sock = UdpSocket::bind("0.0.0.0:8080".parse::<SocketAddr>().unwrap()).await?;

    let (mut tx, mut rx) = mpsc::channel::<(Vec<u8>, SocketAddr)>(1_000);
    let (udp_read, udp_write) = sock.split();
    tokio::spawn(async move {
        while let Some((bytes, addr)) = rx.recv().await {
            let len = udp_write.send_to(&bytes, &addr).await.unwrap();
            println!("{:?} bytes sent", len);
        }
    });

    let mut buf = [0; 1024];
    loop {
        let (len, addr) = udp_read.recv_from(&mut buf).await?;
        println!("{:?} bytes received from {:?}", len, addr);
        tx.send((buf[..len].to_vec(), addr)).await.unwrap();
    }
}
