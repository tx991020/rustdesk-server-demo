


use hbb_common::{ResultType, tokio};
use hbb_common::bytes::Bytes;
use futures::{SinkExt, StreamExt};

use hbb_common::tokio::net::TcpListener;
use hbb_common::tokio_util::codec::{Framed, BytesCodec, LengthDelimitedCodec};



#[tokio::main]
async fn main() -> ResultType<()> {

    let listener = TcpListener::bind("127.0.0.1:9500").await?;
    loop {
        let (stream, addr) = listener.accept().await?;
        println!("accepted: {:?}", addr);
        // LengthDelimitedCodec 默认 4 字节长度
        let mut stream = Framed::new(stream, LengthDelimitedCodec::new());

        tokio::spawn(async move {
            // 接收到的消息会只包含消息主体（不包含长度）
            while let Some(Ok(data)) = stream.next().await {
                println!("Got: {:?}", String::from_utf8_lossy(&data));
                // 发送的消息也需要发送消息主体，不需要提供长度
                // Framed/LengthDelimitedCodec 会自动计算并添加
                stream.send(Bytes::from(data)).await.unwrap();
            }
        });
    }
}