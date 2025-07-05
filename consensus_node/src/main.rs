mod data_structures;
mod p2p;

use data_structures::NodeIdentity;
use std::fs;
use std::path::Path;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Initializing consensus node...");

    let identity = NodeIdentity::new();
    println!("Node Application-Level ID (Public Key Hex): {}", identity.public_key_hex);

    // Create a unique DB path for this node instance to avoid conflicts if running multiple nodes locally
    let db_suffix = &identity.public_key_hex[..8]; // Use first 8 chars of pubkey for a short unique suffix
    let db_path_str = format!("./db_{}", db_suffix);
    let db_path = Path::new(&db_path_str);
    if !db_path.exists() {
        fs::create_dir_all(db_path).expect("Failed to create DB directory");
        println!("Created database directory: {}", db_path_str);
    } else {
        println!("Using existing database directory: {}", db_path_str);
    }


    // Start the P2P communication
    if let Err(e) = p2p::start_node(identity, &db_path_str).await {
        eprintln!("Node failed to start: {}", e);
        std::process::exit(1);
    }

    Ok(())
}
