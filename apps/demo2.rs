use hbb_common::tcp::FramedStream;
use hbb_common::config::{RENDEZVOUS_TIMEOUT, Config};
use hbb_common::to_socket_addr;

fn main() {
    let mut stream = FramedStream::new(to_socket_addr("").unwrap(),
                                       Config::get_any_listen_addr(),
                                       RENDEZVOUS_TIMEOUT,
    )
        .await?;
}