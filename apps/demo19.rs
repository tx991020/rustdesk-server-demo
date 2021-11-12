use hbb_common::bytes::BytesMut;

use hbb_common::tokio;
use hbb_common::tokio::fs::File;
use hbb_common::tokio_util::codec::{BytesCodec, FramedRead};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), std::io::Error> {
    let my_stream_of_bytes = FramedRead::new(my_async_read, BytesCodec::new());
    Ok(())
}
