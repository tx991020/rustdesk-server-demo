fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::compile_protos("proto/helloworld.proto")?;
    tonic_build::compile_protos("proto/message.proto")?;
    tonic_build::compile_protos("proto/rendezvous.proto")?;

    Ok(())
}
