




use tonic::{transport::Server, Request, Response, Status};
use crate::hello_world::greeter_server::{Greeter, GreeterServer};
use crate::hello_world::{HelloReply, HelloRequest};




#[derive(Default)]
pub struct MyGreeter {}

#[tonic::async_trait]
impl Greeter for MyGreeter {
    async fn say_hello(
        &self,
        request: Request<HelloRequest>,
    ) -> Result<Response<HelloReply>, Status> {
        println!("Got a request from {:?}", request.remote_addr());

        let reply = HelloReply {
            message: format!("Hello 456  {}!", request.into_inner().name),
        };
        Ok(Response::new(reply))
    }
}
