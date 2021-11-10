
use hbb_common::futures::{StreamExt, SinkExt};
use hbb_common::tokio_util::codec::BytesCodec;
use hbb_common::tokio;
use hbb_common::tokio::io::BufReader;
use hbb_common::tokio::net::TcpListener;
use hbb_common::tokio::sync::broadcast;
use hbb_common::bytes::Bytes;
use hbb_common::tokio::io::AsyncReadExt;
use hbb_common::tokio::io::{AsyncWriteExt, AsyncBufReadExt};
use hbb_common::tokio_util::codec::Framed;
use hbb_common::bytes::BytesMut;

use crate::broadcast::Sender;


//一对一发

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:13389").await.unwrap();
    let (tx, _) = broadcast::channel(10);


    println!("chat server is ready");
    loop {
        let (mut socket, addr) = listener.accept().await.unwrap();
        println!("clint with addr {} is connected", addr);
        let tx = tx.clone();
        let mut rx = tx.subscribe();

        tokio::spawn(async move {
            let (read, mut write) = socket.split();
            let mut reader = BufReader::new(read);


            let mut line = vec![0; 1024];
            loop {
                tokio::select! {
                    results =  reader.read( &mut line) => {
                        let n = results.unwrap();
                        if n == 0 {
                            println!("good bye :) client {}", &addr);
                            break;
                        }
                        let mut a = BytesMut::from(&line[0..n]);

                        println!("client:{} message: {:?}", &addr, &a);
                        tx.send(( a, addr)).unwrap();


                        line.clear();
                    }
                    results = rx.recv() => {

                        let (message, other_addr) = results.unwrap();
                        if addr != other_addr {
                            write.write_all(&message).await.unwrap();
                        }
                    }
                }
            }
        });
    }
}
