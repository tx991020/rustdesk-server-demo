use hbb_common::tokio::select;
use hbb_common::udp::FramedSocket;
use hbb_common::{to_socket_addr, tokio, ResultType, tcp, Stream};
use std::io;
use hbb_common::tokio_util::codec::{Framed, BytesCodec,LengthDelimitedCodec};
use hbb_common::config::RENDEZVOUS_TIMEOUT;
use hbb_common::tcp::FramedStream;
use hbb_common::anyhow::Context;
use hbb_common::tokio::net::TcpStream;
use hbb_common::futures::{StreamExt,SinkExt};
#[tokio::main]
async fn main() -> ResultType<()> {
    // let mut socket = FramedSocket::new(to_socket_addr("0.0.0.0:0").unwrap()).await?;
    //
    // socket
    //     .send_raw("haha".as_bytes(), to_socket_addr("127.0.0.1:9000").unwrap())
    //     .await?;
    // loop {
    //     select! {
    //         _ = socket.next() => {
    //        println!("4444");
    //         },
    //     }
    // }
    //代理本地的3389
    let listener = tcp::new_listener(format!("0.0.0.0:{}",5000), true).await?;




    loop {
        tokio::select!{
                 Ok((forward, addr)) = listener.accept() => {

                 println!("3333   {:?}",addr)  ;

                //把本地的5000，代理到远程的6000上
                let mut forward = Framed::new(forward, BytesCodec::new());
                       let mut stream = FramedStream::new(to_socket_addr("39.107.33.253:6000").unwrap(), "0.0.0.0:0", RENDEZVOUS_TIMEOUT).await.with_context(|| "Failed to connect to rendezvous server")?;

                tokio::spawn(async move {
                        run_forward(forward,stream)
                    });

            }
            }
    }

    Ok(())
}


async fn run_forward(forward: Framed<TcpStream, BytesCodec>, stream: Stream) -> ResultType<()> {
    println!("{}","ffffff" );

    let mut forward = forward;
    let mut stream = stream;
    loop {
        tokio::select! {
            res = forward.next() => {
                if let Some(Ok(bytes)) = res {
                  stream.send_bytes(bytes.into()).await;
                } else {
                    break;
                }
            },
            res = stream.next() => {
                if let Some(Ok(bytes)) = res {
                    forward.send(bytes.into()).await;
                } else {
                    break;
                }
            },
        }
    }
    Ok(())
}