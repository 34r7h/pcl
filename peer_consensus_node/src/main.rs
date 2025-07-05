use peer_consensus_node::start_node; // Use the library crate name

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // The `env_logger::init()` is called inside `start_node`.
    // If you need any specific setup for the binary before calling start_node,
    // it can be done here.
    start_node().await
}
