use std::env;
use std::error::Error;

use hbb_common::bytes;
use hbb_common::futures::{SinkExt, StreamExt};
use hbb_common::tokio;
use hbb_common::tokio::io;
use hbb_common::tokio::io::AsyncWriteExt;
use hbb_common::tokio::net::{TcpListener, TcpStream};
use hbb_common::tokio_util::codec::{BytesCodec, Framed, LengthDelimitedCodec};
use hbb_common::ResultType;

#[tokio::main]
async fn main() -> ResultType<()> {
    let listen_addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:9000".to_string());
    let server_addr = env::args()
        .nth(2)
        .unwrap_or_else(|| "39.107.33.253:6000".to_string()); //一个websocket连到对端上

    println!("Listening on: {}", listen_addr);
    println!("Proxying to: {}", &server_addr);

    let listener = TcpListener::bind(listen_addr).await?;
    //把9000代理到6000发出去。
    //把6000上的数据转发给对对端的9000 //
    //怎样判断是否对端发来的数据
    //xp编译
    //代理原始信息，websocket广播// 1对1 先注册两端信息,2转发
    //xp编译代理
    //需要在xp编译无客户端的服务并注册,开启3389,把3389转发到21117
    while let Ok((inbound, _)) = listener.accept().await {
        let mut forward = Framed::new(inbound, BytesCodec::new());
        let mut outbound = TcpStream::connect(server_addr.clone()).await?;
        let mut stream = Framed::new(outbound, BytesCodec::new());
        let bytes1 = bytes::Bytes::from("qqqq");
        stream.send(bytes1).await;
        loop {
            tokio::select! {
                res = forward.next() => {
                    if let Some(Ok(bytes)) = res {
                          println!("22222{:?}",&bytes);
                      stream.send(bytes.into()).await;
                    } else {
                        break;
                    }
                },
                res = stream.next() => {
                    if let Some(Ok(bytes)) = res {
                              println!("3333333{:?}",&bytes);
                        forward.send(bytes.into()).await;
                    } else {
                        break;
                    }
                },
            }
        }
    }

    Ok(())
}
