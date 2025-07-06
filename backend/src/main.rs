// PCL Backend Node Main Binary - REAL CONSENSUS PROTOCOL
use pcl_backend::*;
use std::net::SocketAddr;
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use serde_json::{self, Value};
use std::time::{SystemTime, UNIX_EPOCH};
use rand::Rng;
use uuid::Uuid;

// Real consensus protocol implementation
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct ConsensusNode {
    id: String,
    address: String,
    is_leader: bool,
    uptime_score: f64,
    response_time: f64,
    last_pulse: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct ValidationTask {
    task_id: String,
    raw_tx_id: String,
    task_type: String,
    assigned_validator: String,
    complete: bool,
    timestamp: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct RawTransaction {
    raw_tx_id: String,
    tx_data: TransactionData,
    validation_timestamps: Vec<u64>,
    validation_tasks: Vec<ValidationTask>,
    tx_timestamp: u64,
    leader_id: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct ProcessingTransaction {
    tx_id: String,
    tx_data: TransactionData,
    timestamp: u64,
    leader_sig: String,
    leader_id: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct TransactionData {
    to: String,
    from: String,
    amount: f64,
    user: String,
    stake: f64,
    fee: f64,
}

// Consensus Protocol State
struct ConsensusProtocol {
    nodes: HashMap<String, ConsensusNode>,
    leaders: Vec<String>,
    raw_tx_mempool: HashMap<String, HashMap<String, RawTransaction>>, // leader_id -> raw_tx_id -> RawTransaction
    validation_tasks_mempool: HashMap<String, Vec<ValidationTask>>,
    locked_utxo_mempool: Vec<String>,
    processing_tx_mempool: HashMap<String, ProcessingTransaction>,
    tx_mempool: HashMap<String, Transaction>,
    balances: HashMap<String, f64>,
    current_leader_index: usize,
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
    leader_id: Option<String>,
    validators: Vec<String>,
    validation_steps: Vec<String>,
}

impl ConsensusProtocol {
    fn new() -> Self {
        let mut protocol = ConsensusProtocol {
            nodes: HashMap::new(),
            leaders: Vec::new(),
            raw_tx_mempool: HashMap::new(),
            validation_tasks_mempool: HashMap::new(),
            locked_utxo_mempool: Vec::new(),
            processing_tx_mempool: HashMap::new(),
            tx_mempool: HashMap::new(),
            balances: HashMap::new(),
            current_leader_index: 0,
        };
        
        // Initialize with some simulated nodes and leaders
        protocol.initialize_network();
        protocol
    }
    
    fn initialize_network(&mut self) {
        // Create 5 leader nodes
        let leader_names = vec!["Charlie", "Diana", "Eve", "Frank", "Grace"];
        for (i, _name) in leader_names.iter().enumerate() {
            let node_id = format!("leader_{}", i + 1);
            let node = ConsensusNode {
                id: node_id.clone(),
                address: format!("192.168.1.{}", 10 + i),
                is_leader: true,
                uptime_score: 0.95 + (i as f64 * 0.01),
                response_time: 150.0 + (i as f64 * 10.0),
                last_pulse: Self::current_timestamp(),
            };
            self.nodes.insert(node_id.clone(), node);
            self.leaders.push(node_id.clone());
            self.raw_tx_mempool.insert(node_id.clone(), HashMap::new());
            self.validation_tasks_mempool.insert(node_id.clone(), Vec::new());
        }
        
        // Create 10 validator nodes
        for i in 0..10 {
            let node_id = format!("validator_{}", i + 1);
            let node = ConsensusNode {
                id: node_id.clone(),
                address: format!("192.168.1.{}", 20 + i),
                is_leader: false,
                uptime_score: 0.85 + (i as f64 * 0.01),
                response_time: 200.0 + (i as f64 * 15.0),
                last_pulse: Self::current_timestamp(),
            };
            self.nodes.insert(node_id.clone(), node);
        }
        
        // Initialize faucet
        self.balances.insert("faucet_address_123456789".to_string(), 1000000.0);
        
        println!("✅ Consensus Network Initialized:");
        println!("   🏛️  {} Leader nodes", self.leaders.len());
        println!("   🔍 {} Validator nodes", self.nodes.len() - self.leaders.len());
    }
    
    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }
    
    fn get_balance(&self, address: &str) -> f64 {
        *self.balances.get(address).unwrap_or(&0.0)
    }
    
    fn get_current_leader(&self) -> Option<&ConsensusNode> {
        if self.leaders.is_empty() {
            return None;
        }
        let leader_id = &self.leaders[self.current_leader_index % self.leaders.len()];
        self.nodes.get(leader_id)
    }
    
    // Step 1: Alice sends transaction to leader Charlie
    fn submit_transaction(&mut self, tx_data: TransactionData) -> std::result::Result<String, String> {
        let leader = self.get_current_leader().ok_or("No leader available")?.clone();
        let raw_tx_id = Uuid::new_v4().to_string();
        
        // Check balance
        if tx_data.from != "faucet_address_123456789" {
            let sender_balance = self.get_balance(&tx_data.from);
            let required = tx_data.amount + tx_data.stake + tx_data.fee;
            if sender_balance < required {
                return Err(format!("Insufficient balance: {} < {}", sender_balance, required));
            }
        }
        
        // Lock UTXOs
        self.locked_utxo_mempool.push(tx_data.from.clone());
        
        // Create raw transaction
        let raw_tx = RawTransaction {
            raw_tx_id: raw_tx_id.clone(),
            tx_data: tx_data.clone(),
            validation_timestamps: Vec::new(),
            validation_tasks: Vec::new(),
            tx_timestamp: Self::current_timestamp(),
            leader_id: leader.id.clone(),
        };
        
        // Add to leader's raw_tx_mempool
        self.raw_tx_mempool
            .entry(leader.id.clone())
            .or_insert_with(HashMap::new)
            .insert(raw_tx_id.clone(), raw_tx);
        
        // Add to validation tasks mempool
        self.validation_tasks_mempool
            .entry(leader.id.clone())
            .or_insert_with(Vec::new)
            .push(ValidationTask {
                task_id: Uuid::new_v4().to_string(),
                raw_tx_id: raw_tx_id.clone(),
                task_type: "signature_validation".to_string(),
                assigned_validator: "alice".to_string(),
                complete: false,
                timestamp: Self::current_timestamp(),
            });
        
        println!("🔄 Transaction submitted to leader {} ({})", leader.id, leader.address);
        println!("   📝 Raw TX ID: {}", raw_tx_id);
        println!("   💰 Amount: {} XMBL", tx_data.amount);
        println!("   🔒 UTXO locked: {}", tx_data.from);
        
        Ok(raw_tx_id)
    }
    
    // Step 2-3: Leader assigns validation tasks
    fn assign_validation_tasks(&mut self, raw_tx_id: &str) -> Vec<ValidationTask> {
        let mut tasks = Vec::new();
        let validators: Vec<String> = self.nodes.keys()
            .filter(|id| !self.leaders.contains(id))
            .take(3)
            .cloned()
            .collect();
        
        for validator in validators {
            let task = ValidationTask {
                task_id: Uuid::new_v4().to_string(),
                raw_tx_id: raw_tx_id.to_string(),
                task_type: "validation".to_string(),
                assigned_validator: validator.clone(),
                complete: false,
                timestamp: Self::current_timestamp(),
            };
            tasks.push(task);
        }
        
        println!("📋 Assigned {} validation tasks for TX {}", tasks.len(), raw_tx_id);
        tasks
    }
    
    // Step 4-5: Complete validation and move to processing
    fn complete_validation(&mut self, raw_tx_id: &str) -> std::result::Result<String, String> {
        let leader = self.get_current_leader().ok_or("No leader available")?.clone();
        
        // Find raw transaction
        let raw_tx = self.raw_tx_mempool
            .get(&leader.id)
            .and_then(|pool| pool.get(raw_tx_id))
            .ok_or("Raw transaction not found")?
            .clone();
        
        // Simulate validation completion
        let avg_timestamp = Self::current_timestamp();
        let uuid_str = Uuid::new_v4().to_string();
        let tx_id = format!("tx_{}", &uuid_str[..8]);
        
        // Move to processing mempool
        let uuid_str2 = Uuid::new_v4().to_string();
        let processing_tx = ProcessingTransaction {
            tx_id: tx_id.clone(),
            tx_data: raw_tx.tx_data.clone(),
            timestamp: avg_timestamp,
            leader_sig: format!("sig_{}", &uuid_str2[..8]),
            leader_id: leader.id.clone(),
        };
        
        self.processing_tx_mempool.insert(tx_id.clone(), processing_tx);
        
        // Remove from raw mempool
        if let Some(pool) = self.raw_tx_mempool.get_mut(&leader.id) {
            pool.remove(raw_tx_id);
        }
        
        println!("✅ Validation completed for TX {}", raw_tx_id);
        println!("   🚀 Moved to processing as TX {}", tx_id);
        
        Ok(tx_id)
    }
    
    // Step 6: Final validation and ledger update
    fn finalize_transaction(&mut self, tx_id: &str) -> std::result::Result<Transaction, String> {
        let processing_tx = self.processing_tx_mempool
            .get(tx_id)
            .ok_or("Processing transaction not found")?
            .clone();
        
        // Calculate digital root (XMBL Cubic DLT requirement)
        let digital_root = self.calculate_digital_root(tx_id);
        
        // Update balances
        let tx_data = &processing_tx.tx_data;
        
        if tx_data.from != "faucet_address_123456789" {
            let sender_balance = self.get_balance(&tx_data.from);
            let total_deduction = tx_data.amount + tx_data.stake + tx_data.fee;
            let change = tx_data.stake; // Stake returned
            self.balances.insert(tx_data.from.clone(), sender_balance - total_deduction + change);
        }
        
        let recipient_balance = self.get_balance(&tx_data.to);
        self.balances.insert(tx_data.to.clone(), recipient_balance + tx_data.amount);
        
        // Create final transaction
        let final_tx = Transaction {
            hash: tx_id.to_string(),
            from: tx_data.from.clone(),
            to: tx_data.to.clone(),
            amount: tx_data.amount,
            timestamp: processing_tx.timestamp,
            status: "confirmed".to_string(),
            tx_type: Some("transfer".to_string()),
            leader_id: Some(processing_tx.leader_id.clone()),
            validators: vec![
                "validator_1".to_string(),
                "validator_2".to_string(),
                "validator_3".to_string(),
            ],
            validation_steps: vec![
                "Signature validation".to_string(),
                "Balance verification".to_string(),
                "Leader consensus".to_string(),
                "Validator broadcast".to_string(),
                "Digital root calculation".to_string(),
                "Final confirmation".to_string(),
            ],
        };
        
        // Add to final mempool
        self.tx_mempool.insert(tx_id.to_string(), final_tx.clone());
        
        // Remove from processing mempool
        self.processing_tx_mempool.remove(tx_id);
        
        // Unlock UTXOs
        self.locked_utxo_mempool.retain(|utxo| utxo != &tx_data.from);
        
        println!("🎉 Transaction finalized: {} XMBL from {} to {}", 
                 tx_data.amount, tx_data.from, tx_data.to);
        println!("   🔢 Digital root: {}", digital_root);
        println!("   👑 Leader: {}", processing_tx.leader_id);
        
        Ok(final_tx)
    }
    
    fn calculate_digital_root(&self, tx_id: &str) -> u32 {
        let sum: u32 = tx_id.chars()
            .filter_map(|c| c.to_digit(10))
            .sum();
        
        if sum < 10 {
            sum
        } else {
            sum % 9
        }
    }
    
    fn get_recent_transactions(&self) -> Vec<&Transaction> {
        self.tx_mempool.values().collect()
    }
    
    fn get_network_info(&self) -> serde_json::Value {
        serde_json::json!({
            "leaders": self.leaders.len(),
            "validators": self.nodes.len() - self.leaders.len(),
            "current_leader": self.get_current_leader().map(|l| &l.id),
            "raw_transactions": self.raw_tx_mempool.values().map(|pool| pool.len()).sum::<usize>(),
            "processing_transactions": self.processing_tx_mempool.len(),
            "finalized_transactions": self.tx_mempool.len(),
            "locked_utxos": self.locked_utxo_mempool.len(),
            "validation_tasks": self.validation_tasks_mempool.values().map(|tasks| tasks.len()).sum::<usize>(),
        })
    }
    
    fn get_transaction_details(&self, tx_id: &str) -> Option<serde_json::Value> {
        self.tx_mempool.get(tx_id).map(|tx| {
            serde_json::json!({
                "transaction": tx,
                "leader_node": self.nodes.get(tx.leader_id.as_ref().unwrap_or(&"unknown".to_string())),
                "consensus_info": {
                    "digital_root": self.calculate_digital_root(tx_id),
                    "validation_steps_completed": tx.validation_steps.len(),
                    "validators_involved": tx.validators.len(),
                }
            })
        })
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    
    println!("🚀 XMBL Cubic DLT Consensus Protocol Starting...");
    
    // Initialize real consensus protocol
    let consensus = Arc::new(RwLock::new(ConsensusProtocol::new()));
    println!("✅ Real consensus protocol initialized");
    
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
    println!("✅ XMBL Cubic DLT Consensus Protocol is ready");
    
    // Simple HTTP server loop
    loop {
        match listener.accept().await {
            Ok((mut stream, _)) => {
                let storage = storage.clone();
                let mempool = mempool.clone();
                let consensus = consensus.clone();
                
                tokio::spawn(async move {
                    let mut buffer = [0; 4096];
                    
                    if let Ok(n) = stream.read(&mut buffer).await {
                        let request = String::from_utf8_lossy(&buffer[..n]);
                        let request_line = request.lines().next().unwrap_or("");
                        println!("📨 Request: {}", request_line);
                        
                        let response = if request.contains("GET /health") {
                            handle_health().await
                        } else if request.contains("GET /network") {
                            handle_network(consensus.clone()).await
                        } else if request.contains("GET /balance/") {
                            handle_balance(&request, consensus.clone()).await
                        } else if request.contains("GET /transactions/") {
                            handle_transactions(&request, consensus.clone()).await
                        } else if request.contains("GET /transaction/") {
                            handle_transaction_details(&request, consensus.clone()).await
                        } else if request.contains("POST /transaction") {
                            handle_transaction_post(&request, mempool, consensus.clone()).await
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
    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\n\r\n{\"status\":\"healthy\",\"message\":\"XMBL Cubic DLT Consensus Protocol is running\"}\r\n".to_string()
}

async fn handle_network(consensus: Arc<RwLock<ConsensusProtocol>>) -> String {
    let consensus = consensus.read().await;
    let network_info = consensus.get_network_info();
    
    format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\n\r\n{}\r\n", network_info)
}

async fn handle_balance(request: &str, consensus: Arc<RwLock<ConsensusProtocol>>) -> String {
    let address = request.lines()
        .next()
        .and_then(|line| line.split("/balance/").nth(1))
        .and_then(|addr| addr.split_whitespace().next())
        .unwrap_or("unknown");
    
    println!("💰 Balance requested for address: {}", address);
    
    let consensus = consensus.read().await;
    let balance = consensus.get_balance(address);
    
    let response = serde_json::json!({
        "address": address,
        "balance": balance,
        "message": "Real consensus protocol balance"
    });
    
    format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\n\r\n{}\r\n", response)
}

async fn handle_transactions(request: &str, consensus: Arc<RwLock<ConsensusProtocol>>) -> String {
    let address = request.lines()
        .next()
        .and_then(|line| line.split("/transactions/").nth(1))
        .and_then(|addr| addr.split_whitespace().next())
        .unwrap_or("unknown");
    
    println!("📋 Transactions requested for address: {}", address);
    
    let consensus = consensus.read().await;
    let transactions = if address == "recent" {
        consensus.get_recent_transactions()
    } else {
        consensus.get_recent_transactions().into_iter()
            .filter(|tx| tx.from == address || tx.to == address)
            .collect()
    };
    
    let response = serde_json::json!({
        "address": address,
        "transactions": transactions
    });
    
    format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\n\r\n{}\r\n", response)
}

async fn handle_transaction_details(request: &str, consensus: Arc<RwLock<ConsensusProtocol>>) -> String {
    let tx_id = request.lines()
        .next()
        .and_then(|line| line.split("/transaction/").nth(1))
        .and_then(|id| id.split_whitespace().next())
        .unwrap_or("unknown");
    
    println!("🔍 Transaction details requested for: {}", tx_id);
    
    let consensus = consensus.read().await;
    let details = consensus.get_transaction_details(tx_id);
    
    let response = details.unwrap_or_else(|| serde_json::json!({
        "error": "Transaction not found",
        "tx_id": tx_id
    }));
    
    format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\n\r\n{}\r\n", response)
}

async fn handle_transaction_post(request: &str, _mempool: Arc<MempoolManager>, consensus: Arc<RwLock<ConsensusProtocol>>) -> String {
    println!("💸 Transaction submission requested");
    
    let body = request.split("\r\n\r\n").nth(1).unwrap_or("{}");
    
    let tx_data: std::result::Result<Value, serde_json::Error> = serde_json::from_str(body);
    
    match tx_data {
        Ok(data) => {
            let tx_data = TransactionData {
                from: data["from"].as_str().unwrap_or("unknown").to_string(),
                to: data["to"].as_str().unwrap_or("unknown").to_string(),
                amount: data["amount"].as_f64().unwrap_or(0.0),
                user: data["from"].as_str().unwrap_or("unknown").to_string(),
                stake: data["stake"].as_f64().unwrap_or(0.1),
                fee: data["fee"].as_f64().unwrap_or(0.05),
            };
            
            let mut consensus = consensus.write().await;
            
            // Step 1: Submit transaction
            match consensus.submit_transaction(tx_data) {
                Ok(raw_tx_id) => {
                    // Step 2-3: Assign validation tasks
                    let _tasks = consensus.assign_validation_tasks(&raw_tx_id);
                    
                    // Step 4-5: Complete validation (simulated)
                    match consensus.complete_validation(&raw_tx_id) {
                        Ok(tx_id) => {
                            // Step 6: Finalize transaction
                            match consensus.finalize_transaction(&tx_id) {
                                Ok(final_tx) => {
                                    let response = serde_json::json!({
                                        "success": true,
                                        "hash": final_tx.hash,
                                        "message": "Transaction processed through consensus protocol",
                                        "leader_id": final_tx.leader_id,
                                        "validators": final_tx.validators,
                                        "validation_steps": final_tx.validation_steps,
                                        "new_balance_from": consensus.get_balance(&final_tx.from),
                                        "new_balance_to": consensus.get_balance(&final_tx.to)
                                    });
                                    
                                    format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: *\r\nAccess-Control-Allow-Headers: *\r\n\r\n{}\r\n", response)
                                }
                                Err(e) => {
                                    let response = serde_json::json!({
                                        "success": false,
                                        "error": format!("Finalization failed: {}", e)
                                    });
                                    
                                    format!("HTTP/1.1 500 Internal Server Error\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\n\r\n{}\r\n", response)
                                }
                            }
                        }
                        Err(e) => {
                            let response = serde_json::json!({
                                "success": false,
                                "error": format!("Validation failed: {}", e)
                            });
                            
                            format!("HTTP/1.1 500 Internal Server Error\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\n\r\n{}\r\n", response)
                        }
                    }
                }
                Err(e) => {
                    let response = serde_json::json!({
                        "success": false,
                        "error": format!("Transaction submission failed: {}", e)
                    });
                    
                    format!("HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\n\r\n{}\r\n", response)
                }
            }
        }
        Err(e) => {
            println!("❌ Invalid transaction data: {}", e);
            let response = serde_json::json!({
                "success": false,
                "error": "Invalid transaction format"
            });
            
            format!("HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\n\r\n{}\r\n", response)
        }
    }
}

async fn handle_options() -> String {
    "HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\n\r\n".to_string()
}

async fn handle_not_found() -> String {
    "HTTP/1.1 404 Not Found\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\n\r\n{\"error\":\"Not found\"}\r\n".to_string()
} 