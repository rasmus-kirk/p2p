fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::compile_protos("grpc/p2p.proto")?;
    Ok(())
}
