use hbb_common::{tokio, timeout, to_socket_addr};


use hbb_common::tokio::net::{TcpListener, TcpStream};
use hbb_common::ResultType;
use hbb_common::bytes::Bytes;
use hbb_common::tokio_util::codec::{Framed, BytesCodec};

use hbb_common::config::RENDEZVOUS_TIMEOUT;
use hbb_common::anyhow::Context;
use hbb_common::futures::{StreamExt,SinkExt};

#[tokio::main]
async fn main() -> ResultType<()> {
    // let listen_addr = env::args()
    //     .nth(1)
    //     .unwrap_or_else(|| "127.0.0.1:8081".to_string());
    // let server_addr = env::args()
    //     .nth(2)
    //     .unwrap_or_else(|| "127.0.0.1:13000".to_string());
    //
    // println!("Listening on: {}", listen_addr);
    // println!("Proxying to: {}", server_addr);
    tcp3().await;


    Ok(())
}

async fn tcp1()->ResultType<()>{
    let listener = TcpListener::bind("0.0.0.0:3389").await?;
    while let Ok((inbound, _)) = listener.accept().await {
        // let transfer = transfer(inbound, server_addr.clone()).map(|r| {
        //     if let Err(e) = r {
        //         println!("Failed to transfer; error={}", e);
        //     }
        // });

        // tokio::spawn(transfer(inbound,server_addr.clone()));
    }

    Ok(())
}


async fn tcp2()->ResultType<()>{


    let listener = TcpListener::bind("0.0.0.0:3389").await?;
    while let Ok((inbound, _)) = listener.accept().await {
        // let transfer = transfer(inbound, server_addr.clone()).map(|r| {
        //     if let Err(e) = r {
        //         println!("Failed to transfer; error={}", e);
        //     }
        // });

        // tokio::spawn(transfer(inbound,server_addr.clone()));
    }

    Ok(())

}


async fn tcp3(){

    //把本地3389的数据代理到远程6000上，端口复用
    //远程6000 tokio broadcast


    let mut stream1 = TcpStream::connect("39.107.33.253:6000").await.unwrap();


    // println!("{:?}",socket.get_ref().local_addr() );
    let mut stream =Framed::new(stream1, BytesCodec::new());
    let sock = TcpStream::connect("127.0.0.1:3389").await.unwrap();
    let mut forward = Framed::new(sock, BytesCodec::new());


    loop {
        tokio::select!{

             res = forward.next() =>{
                 if let  Some(Ok(bytes)) = res {
                     println!("本地3389发出去{:?}",&bytes);

                     stream.send(Bytes::from(bytes)).await;
                }else {
                           println!("{}",3333);
                            break;

                        }

            }
            res = stream.next() => {
                   if let  Some(Ok(bytes)) = res {
                     println!("44444 远程返回的发给3389{:?}",&bytes);

                     stream.send(Bytes::from(bytes)).await;
                }else {
                           println!("{}",555);
                            break;

                        }
            }
        }
    }


}
// async fn transfer(mut inbound: TcpStream, mut outbound:  TcpStream) -> Result<(), Box<dyn Error>> {
//
//
//     let (mut ri, mut wi) = inbound.split();
//     let (mut ro, mut wo) = outbound.split();
//
//     let client_to_server = async {
//         io::copy(&mut ri, &mut wo).await?;
//         wo.shutdown().await
//     };
//
//     let server_to_client = async {
//         io::copy(&mut ro, &mut wi).await?;
//         wi.shutdown().await
//     };
//
//     tokio::try_join!(client_to_server, server_to_client)?;
//
//     Ok(())
// }
