
use hbb_common::bytes::Bytes;
use hbb_common::futures::{StreamExt,SinkExt};
use hbb_common::tokio_util::codec::{Framed,LengthDelimitedCodec};
use hbb_common::tokio;
use hbb_common::tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
    sync::broadcast,
};
use lazy_static::lazy_static;
use dashmap::DashMap;
use std::sync::Arc;


#[derive(Debug, Clone)]
pub struct Client {
    //心跳
    pub timestamp: u64,
    pub local_addr: String,
    pub peer_addr:String,
    //内网地址
    pub uuid: String,
}


lazy_static!{
    pub static ref IdMap: Arc<DashMap<String, Client>> = Arc::new(DashMap::new());
}




//注册自己和对方
#[tokio::main]
async fn main() {


    let listener = TcpListener::bind("127.0.0.1:13000").await.unwrap();

    let (tx, _rx) = broadcast::channel(10);

    loop {
        let (mut socket, addr) = listener.accept().await.unwrap();

        let tx = tx.clone();
        let mut rx = tx.subscribe();
        let mut stream = Framed::new(socket, LengthDelimitedCodec::new());

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(Ok(data)) = stream.next()  => {

                        tx.send((data, addr)).unwrap();


                    }
                    result = rx.recv() => {
                        let (messg, other_addr) = result.unwrap();
                        println!("{:?},{:?}",&messg,&other_addr);
                       if addr !=other_addr {
                             stream.send(Bytes::from(messg)).await.unwrap();
                        }

                    }
                }
            }
        });
    }
}
