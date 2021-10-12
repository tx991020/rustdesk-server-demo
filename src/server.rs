
use hbb_common::tokio;
use hbb_common::anyhow::Result;
use tonic::transport::Server;

use rustdesk_server::GreeterServer;
use rustdesk_server::controller::hello::MyGreeter;


#[tokio::main]
async fn main() ->Result<()>{
    let addr = "0.0.0.0:14000".parse().unwrap();
    println!("Server listening on: {}", addr);
    let greeter = MyGreeter::default();
    Server::builder()
        .add_service(GreeterServer::new(greeter))
        .serve(addr)
        .await?;

    Ok(())


}