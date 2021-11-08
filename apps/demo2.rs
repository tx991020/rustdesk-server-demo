
use hbb_common::ResultType;
use hbb_common::bytes::Bytes;
use hbb_common::futures::{SinkExt, StreamExt};
use hbb_common::tokio;
use hbb_common::tokio_util::codec::{Framed, LengthDelimitedCodec};
use hbb_common::tokio::io::{AsyncWriteExt,AsyncReadExt};
use hbb_common::tokio::net::TcpListener;
use hbb_common::tokio::sync::broadcast;

#[tokio::main]
async fn main() -> ResultType<()> {
    let listener = TcpListener::bind("127.0.0.1:9520").await?;
    let (tx, _rx) = broadcast::channel(10);
    loop {
        let tx = tx.clone();
        let mut rx = tx.subscribe();

        let (mut stream, addr) = listener.accept().await?;
        println!("accepted: {:?}", addr);
        // LengthDelimitedCodec 默认 4 字节长度
        let mut stream = Framed::new(stream, LengthDelimitedCodec::new());
        //
        // tokio::spawn(async move {
        //     // 接收到的消息会只包含消息主体（不包含长度）
        //     while let Some(Ok(data)) = stream.next().await {
        //         println!("Got: {:?}", String::from_utf8_lossy(&data));
        //         // 发送的消息也需要发送消息主体，不需要提供长度
        //         // Framed/LengthDelimitedCodec 会自动计算并添加
        //         // stream.send(Bytes::from(data)).await.unwrap();
        //         tx.send((line.clone(), addr)).unwrap();
        //     }
        // });

        tokio::spawn(async move {
            loop {
                tokio::select! {
                   Some(Ok(data)) = stream.next() => {

                        tx.send((data, addr)).unwrap();

                    }
                    result = rx.recv() => {
                        let (messg, other_addr) = result.unwrap();
                        println!("{:?},{:?}",&messg,&other_addr);

                        if addr != other_addr {
                            writer.write_all(messg.as_bytes()).await.unwrap();
                        }
                    }
                }
            }
        })
    }
    Ok(())
}

