use hbb_common::tokio::select;
use hbb_common::udp::FramedSocket;
use hbb_common::{to_socket_addr, tokio, ResultType};
use std::collections::HashSet;
use std::io;

//udp_chat_demo
#[tokio::main]
async fn main() -> ResultType<()> {
    let mut socket = FramedSocket::new(to_socket_addr("127.0.0.1:9000").unwrap()).await?;
    let mut set = HashSet::new();
    loop {
        select! {
            Some(Ok((_, addr))) = socket.next() => {
                set.insert(addr);
            println!("{}","1111111");
            for i in set.iter() {
                     println!("3333{}",&addr);
                if i != &addr{
                        println!("2222{:?}",i);
                socket.send_raw("hahhha".as_bytes(), i.clone()).await?;
                }
            }
            },
        }
    }

    Ok(())
}
