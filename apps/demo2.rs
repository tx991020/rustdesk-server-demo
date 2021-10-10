use hbb_common::config::{Config, RENDEZVOUS_TIMEOUT};
use hbb_common::tcp::FramedStream;
use hbb_common::tokio;
use hbb_common::{to_socket_addr, ResultType};

#[tokio::main]
async fn main() -> ResultType<()> {
    let mut stream = FramedStream::new(
        to_socket_addr("127.0.0.1:20000").unwrap(),
        Config::get_any_listen_addr(),
        RENDEZVOUS_TIMEOUT,
    )
    .await?;

    stream.send_raw("hahha".as_bytes().to_vec()).await?;
    Ok(())
}
