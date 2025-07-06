// PCL Backend Node Main Binary
use pcl_backend::*;
use std::net::SocketAddr;
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use serde_json::{self, Value};

// Real ledger to track balances and transactions
struct Ledger {
    balances: HashMap<String, f64>,
    transactions: Vec<Transaction>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Transaction {
    hash: String,
    from: String,
    to: String,
    amount: f64,
    timestamp: u64,
    status: String,
    tx_type: Option<String>,
}

impl Ledger {
    fn new() -> Self {
        let mut ledger = Ledger {
            balances: HashMap::new(),
            transactions: Vec::new(),
        };
        
        // Initialize faucet with funds
        ledger.balances.insert("faucet_address_123456789".to_string(), 1000000.0);
        ledger
    }
    
    fn get_balance(&self, address: &str) -> f64 {
        *self.balances.get(address).unwrap_or(&0.0)
    }
    
    fn process_transaction(&mut self, tx: Transaction) -> bool {
        println!("🔄 Processing transaction: {} XMBL from {} to {}", tx.amount, tx.from, tx.to);
        
        // Check if sender has sufficient balance (except for faucet)
        if tx.from != "faucet_address_123456789" {
            let sender_balance = self.get_balance(&tx.from);
            if sender_balance < tx.amount {
                println!("❌ Insufficient balance: {} < {}", sender_balance, tx.amount);
                return false;
            }
        }
        
        // Process the transaction
        if tx.from != "faucet_address_123456789" {
            let sender_balance = self.get_balance(&tx.from);
            self.balances.insert(tx.from.clone(), sender_balance - tx.amount);
        }
        
        let recipient_balance = self.get_balance(&tx.to);
        self.balances.insert(tx.to.clone(), recipient_balance + tx.amount);
        
        // Store transaction
        self.transactions.push(tx);
        
        println!("✅ Transaction processed successfully");
        true
    }
    
    fn get_transactions(&self, address: &str) -> Vec<&Transaction> {
        self.transactions.iter()
            .filter(|tx| tx.from == address || tx.to == address)
            .collect()
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    
    println!("🚀 PCL Node starting...");
    
    // Initialize real ledger
    let ledger = Arc::new(RwLock::new(Ledger::new()));
    println!("✅ Real ledger initialized");
    
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
                let ledger = ledger.clone();
                
                tokio::spawn(async move {
                    let mut buffer = [0; 2048];
                    
                    if let Ok(n) = stream.read(&mut buffer).await {
                        let request = String::from_utf8_lossy(&buffer[..n]);
                        let request_line = request.lines().next().unwrap_or("");
                        println!("📨 Request: {}", request_line);
                        
                        let response = if request.contains("GET /health") {
                            handle_health().await
                        } else if request.contains("GET /balance/") {
                            handle_balance(&request, ledger.clone()).await
                        } else if request.contains("GET /transactions/") {
                            handle_transactions(&request, ledger.clone()).await
                        } else if request.contains("POST /transaction") {
                            handle_transaction_post(&request, mempool, ledger.clone()).await
                        } else if request.contains("OPTIONS") {
                            handle_options().await
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

async fn handle_balance(request: &str, ledger: Arc<RwLock<Ledger>>) -> String {
    // Extract address from URL
    let address = request.lines()
        .next()
        .and_then(|line| line.split("/balance/").nth(1))
        .and_then(|addr| addr.split_whitespace().next())
        .unwrap_or("unknown");
    
    println!("💰 Balance requested for address: {}", address);
    
    let ledger = ledger.read().await;
    let balance = ledger.get_balance(address);
    
    let response = serde_json::json!({
        "address": address,
        "balance": balance,
        "message": "Real balance from ledger"
    });
    
    format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\n\r\n{}\r\n", response)
}

async fn handle_transactions(request: &str, ledger: Arc<RwLock<Ledger>>) -> String {
    let address = request.lines()
        .next()
        .and_then(|line| line.split("/transactions/").nth(1))
        .and_then(|addr| addr.split_whitespace().next())
        .unwrap_or("unknown");
    
    println!("📋 Transactions requested for address: {}", address);
    
    let ledger = ledger.read().await;
    let transactions = if address == "recent" {
        ledger.transactions.iter().rev().take(10).collect::<Vec<_>>()
    } else {
        ledger.get_transactions(address)
    };
    
    let response = serde_json::json!({
        "address": address,
        "transactions": transactions
    });
    
    format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\n\r\n{}\r\n", response)
}

async fn handle_transaction_post(request: &str, _mempool: Arc<MempoolManager>, ledger: Arc<RwLock<Ledger>>) -> String {
    println!("💸 Transaction submission requested");
    
    // Parse transaction from request body
    let body = request.split("\r\n\r\n").nth(1).unwrap_or("{}");
    
    let tx_data: std::result::Result<Value, serde_json::Error> = serde_json::from_str(body);
    
    match tx_data {
        Ok(data) => {
            let tx = Transaction {
                hash: format!("tx_{}", uuid::Uuid::new_v4().to_string()[..8].to_string()),
                from: data["from"].as_str().unwrap_or("unknown").to_string(),
                to: data["to"].as_str().unwrap_or("unknown").to_string(),
                amount: data["amount"].as_f64().unwrap_or(0.0),
                timestamp: data["timestamp"].as_u64().unwrap_or(0),
                status: "confirmed".to_string(),
                tx_type: data["type"].as_str().map(|s| s.to_string()),
            };
            
            let mut ledger = ledger.write().await;
            let success = ledger.process_transaction(tx.clone());
            
            if success {
                let response = serde_json::json!({
                    "success": true,
                    "hash": tx.hash,
                    "message": "Transaction processed successfully",
                    "new_balance_from": ledger.get_balance(&tx.from),
                    "new_balance_to": ledger.get_balance(&tx.to)
                });
                
                format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: *\r\nAccess-Control-Allow-Headers: *\r\n\r\n{}\r\n", response)
            } else {
                let response = serde_json::json!({
                    "success": false,
                    "error": "Transaction failed - insufficient balance"
                });
                
                format!("HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: *\r\nAccess-Control-Allow-Headers: *\r\n\r\n{}\r\n", response)
            }
        }
        Err(e) => {
            println!("❌ Invalid transaction data: {}", e);
            let response = serde_json::json!({
                "success": false,
                "error": "Invalid transaction format"
            });
            
            format!("HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: *\r\nAccess-Control-Allow-Headers: *\r\n\r\n{}\r\n", response)
        }
    }
}

async fn handle_options() -> String {
    "HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\n\r\n".to_string()
}

async fn handle_not_found() -> String {
    "HTTP/1.1 404 Not Found\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\n\r\n{\"error\":\"Not found\"}\r\n".to_string()
} 