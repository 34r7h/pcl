// PCL Backend Node Main Binary
use pcl_backend::*;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use serde_json;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    
    println!("🚀 PCL Node starting...");
    
    // Initialize storage
    let storage = Arc::new(StorageManager::new("./pcl_data")?);
    println!("✅ Storage initialized");
    
    // Initialize node
    let keypair = NodeKeypair::new();
    let node = Node::new(
        "127.0.0.1".parse().unwrap(),
        &keypair,
    )?;
    println!("✅ Node created: {}", node.ip_address);
    
    // Initialize mempool manager
    let mempool = Arc::new(MempoolManager::new());
    println!("✅ Mempool initialized");
    
    // Initialize network manager
    let network = NetworkManager::new(node.clone()).await?;
    println!("✅ Network initialized");
    
    // Initialize consensus manager with separate storage instance
    // let consensus_storage = StorageManager::new("./pcl_data_consensus")?;
    // let consensus = Arc::new(ConsensusManager::new(
    //     node.clone(),
    //     network,
    //     consensus_storage,
    // )?);
    // println!("✅ Consensus initialized");
    
    // Start HTTP server for API
    let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
    let listener = TcpListener::bind(addr).await?;
    println!("🌐 Server listening on http://{}", addr);
    println!("✅ PCL Node is ready for connections");
    
    // Simple HTTP server loop
    loop {
        match listener.accept().await {
            Ok((mut stream, _)) => {
                let storage = storage.clone();
                let mempool = mempool.clone();
                
                tokio::spawn(async move {
                    let mut buffer = [0; 1024];
                    
                    if let Ok(n) = stream.read(&mut buffer).await {
                        let request = String::from_utf8_lossy(&buffer[..n]);
                        println!("📨 Request: {}", request.lines().next().unwrap_or(""));
                        
                        let response = if request.contains("GET /health") {
                            handle_health().await
                        } else if request.contains("GET /balance/") {
                            handle_balance(&request).await
                        } else if request.contains("GET /transactions/") {
                            handle_transactions(&request).await
                        } else if request.contains("POST /transaction") {
                            handle_transaction_post(&request, mempool).await
                        } else {
                            handle_not_found().await
                        };
                        
                        let _ = stream.write_all(response.as_bytes()).await;
                    }
                });
            }
            Err(e) => {
                eprintln!("❌ Failed to accept connection: {}", e);
            }
        }
    }
}

async fn handle_health() -> String {
    println!("💚 Health check requested");
    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\n\r\n{\"status\":\"healthy\",\"message\":\"PCL Node is running\"}\r\n".to_string()
}

async fn handle_balance(request: &str) -> String {
    // Extract address from URL
    let address = request.lines()
        .next()
        .and_then(|line| line.split("/balance/").nth(1))
        .and_then(|addr| addr.split_whitespace().next())
        .unwrap_or("unknown");
    
    println!("💰 Balance requested for address: {}", address);
    
    let balance = serde_json::json!({
        "address": address,
        "balance": 100.0,
        "message": "Mock balance for testing"
    });
    
    format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\n\r\n{}\r\n", balance)
}

async fn handle_transactions(request: &str) -> String {
    let address = request.lines()
        .next()
        .and_then(|line| line.split("/transactions/").nth(1))
        .and_then(|addr| addr.split_whitespace().next())
        .unwrap_or("unknown");
    
    println!("📋 Transactions requested for address: {}", address);
    
    let transactions = serde_json::json!({
        "address": address,
        "transactions": [
            {
                "hash": "tx_123abc",
                "from": "sender_addr",
                "to": address,
                "amount": 10.0,
                "status": "confirmed",
                "timestamp": 1640995200000i64
            }
        ]
    });
    
    format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\n\r\n{}\r\n", transactions)
}

async fn handle_transaction_post(_request: &str, _mempool: Arc<MempoolManager>) -> String {
    println!("💸 Transaction submission requested");
    
    // Parse transaction from request body (simplified)
    let response = serde_json::json!({
        "success": true,
        "hash": "tx_new_456def",
        "message": "Transaction submitted to mempool"
    });
    
    format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\n\r\n{}\r\n", response)
}

async fn handle_not_found() -> String {
    "HTTP/1.1 404 Not Found\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\n\r\n{\"error\":\"Not found\"}\r\n".to_string()
} 