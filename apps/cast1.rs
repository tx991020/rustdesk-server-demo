use dashmap::DashMap;
use hbb_common::bytes::Bytes;
use hbb_common::futures::{SinkExt, StreamExt};
use hbb_common::tokio::io::AsyncReadExt;
use hbb_common::tokio::io::BufReader;
use hbb_common::tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use hbb_common::tokio::net::TcpListener;
use hbb_common::tokio::sync::broadcast;
use hbb_common::tokio::time::interval;
use hbb_common::tokio_util::codec::Framed;
use hbb_common::tokio_util::codec::{BytesCodec, LengthDelimitedCodec};
use hbb_common::{tokio, ResultType};
use lazy_static::lazy_static;
use std::sync::Arc;
use std::time::Duration;

//一对一发

lazy_static::lazy_static! {

    pub static ref IdMap: Arc<DashMap<String, Client>> = Arc::new(DashMap::new());

}

//删除30秒内没心跳的号
async fn traverse_ip_map(id_map: Arc<DashMap<String, Client>>) {
    let mut interval = interval(Duration::from_secs(20));
    loop {
        let mut guard = id_map.lock().await;
        // println!("在线用户{:#?}", guard);
        guard.retain(|key, value| value.timestamp > (get_time() - 1000 * 20) as u64);
        drop(guard);
        interval.tick().await;
    }
}

#[derive(Debug, Clone)]
pub struct Client {
    //心跳
    pub timestamp: u64,
    pub local_addr: String,
    pub peer_addr: String,
    //内网地址
    pub uuid: String,
}

#[tokio::main]
async fn main() -> ResultType<()> {
    cast().await;
    Ok(())
}

async fn register() {

}

async fn cast() -> ResultType<()> {
    let listener = TcpListener::bind("127.0.0.1:13389").await?;
    let (tx, _) = broadcast::channel(10);

    println!("chat server is ready");
    loop {
        let (mut socket, addr) = listener.accept().await?;
        println!("clint with addr {} is connected", addr);
        let tx = tx.clone();
        let mut rx = tx.subscribe();

        let mut stream = Framed::new(socket, BytesCodec::new());
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    results =  stream.next() => {
                     if let Some(Ok(bytes)) = results{
                             println!(" xxx {:?}",bytes);
                               tx.send((Bytes::from(bytes), addr.ip().to_string())).unwrap();
                        }else {
                            println!("{}",333);
                            break;
                        }
                    }
                    results = rx.recv() => {
                        let (message, other_addr) = results.unwrap();
                       let option = IdMap.get(addr.ip().to_string());
                        if option.is_some() {
                           if  option.unwrap().peer_addr =other_addr{
                                 stream.send(message).await.unwrap();
                            }

                        }

                    }
                }
            }
        });
    }
    Ok(())
}
