

pub mod hello_world {
    tonic::include_proto!("helloworld");
}
pub use hello_world::greeter_server::{Greeter, GreeterServer};
pub use hello_world::{HelloReply, HelloRequest};


pub mod version;
pub use version::*;
pub mod controller;

pub mod config;