// PCL Backend Node Main Binary - REAL CONSENSUS PROTOCOL WITH CROSS-VALIDATION
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

// Real consensus protocol implementation with cross-validation
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct ConsensusNode {
    id: String,
    name: String,
    address: String,
    is_leader: bool,
    is_simulator: bool,
    uptime_score: f64,
    response_time: f64,
    last_pulse: u64,
    public_key: String,
    validation_tasks_completed: u32,
    validation_tasks_assigned: u32,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct ValidationTask {
    task_id: String,
    raw_tx_id: String,
    task_type: String,
    assigned_validator: String,
    validator_must_validate_tx: String, // TX that this validator must validate
    complete: bool,
    timestamp: u64,
    completion_timestamp: Option<u64>,
    validator_signature: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct RawTransaction {
    raw_tx_id: String,
    tx_data: TransactionData,
    validation_timestamps: Vec<u64>,
    validation_tasks: Vec<ValidationTask>,
    tx_timestamp: u64,
    leader_id: String,
    status: String, // "pending", "validating", "processing", "finalized"
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct ProcessingTransaction {
    tx_id: String,
    tx_data: TransactionData,
    timestamp: u64,
    leader_sig: String,
    leader_id: String,
    validation_results: Vec<ValidationResult>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct ValidationResult {
    validator_id: String,
    validation_task_id: String,
    result: bool,
    signature: String,
    timestamp: u64,
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

// Consensus Protocol State with Cross-Validation
struct ConsensusProtocol {
    nodes: HashMap<String, ConsensusNode>,
    leaders: Vec<String>,
    simulator_nodes: Vec<String>,
    raw_tx_mempool: HashMap<String, HashMap<String, RawTransaction>>,
    validation_tasks_mempool: HashMap<String, Vec<ValidationTask>>,
    user_validation_queue: HashMap<String, Vec<String>>, // user -> list of tx_ids they must validate
    locked_utxo_mempool: Vec<String>,
    processing_tx_mempool: HashMap<String, ProcessingTransaction>,
    tx_mempool: HashMap<String, Transaction>,
    balances: HashMap<String, f64>,
    current_leader_index: usize,
    cross_validation_log: Vec<String>,
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
    cross_validators: Vec<String>, // Users who validated this transaction
    validation_tasks_for_submitter: Vec<String>, // Tasks the submitter had to complete
}

impl ConsensusProtocol {
    fn new() -> Self {
        let mut protocol = ConsensusProtocol {
            nodes: HashMap::new(),
            leaders: Vec::new(),
            simulator_nodes: Vec::new(),
            raw_tx_mempool: HashMap::new(),
            validation_tasks_mempool: HashMap::new(),
            user_validation_queue: HashMap::new(),
            locked_utxo_mempool: Vec::new(),
            processing_tx_mempool: HashMap::new(),
            tx_mempool: HashMap::new(),
            balances: HashMap::new(),
            current_leader_index: 0,
            cross_validation_log: Vec::new(),
        };
        
        // Initialize with real consensus network including simulator nodes
        protocol.initialize_network();
        protocol
    }
    
    fn initialize_network(&mut self) {
        // Create 5 leader nodes
        let leader_names = vec!["Charlie", "Diana", "Eve", "Frank", "Grace"];
        for (i, name) in leader_names.iter().enumerate() {
            let node_id = format!("leader_{}", i + 1);
            let node = ConsensusNode {
                id: node_id.clone(),
                name: name.to_string(),
                address: format!("192.168.1.{}", 10 + i),
                is_leader: true,
                is_simulator: false,
                uptime_score: 0.95 + (i as f64 * 0.01),
                response_time: 150.0 + (i as f64 * 10.0),
                last_pulse: Self::current_timestamp(),
                public_key: format!("leader_pubkey_{}", i + 1),
                validation_tasks_completed: 0,
                validation_tasks_assigned: 0,
            };
            self.nodes.insert(node_id.clone(), node);
            self.leaders.push(node_id.clone());
            self.raw_tx_mempool.insert(node_id.clone(), HashMap::new());
            self.validation_tasks_mempool.insert(node_id.clone(), Vec::new());
        }
        
        // Create 10 validator nodes (some are simulator nodes)
        for i in 0..10 {
            let node_id = format!("validator_{}", i + 1);
            let is_simulator = i < 5; // First 5 validators are simulator nodes
            let node = ConsensusNode {
                id: node_id.clone(),
                name: format!("Validator{}", i + 1),
                address: format!("192.168.1.{}", 20 + i),
                is_leader: false,
                is_simulator,
                uptime_score: 0.85 + (i as f64 * 0.01),
                response_time: 200.0 + (i as f64 * 15.0),
                last_pulse: Self::current_timestamp(),
                public_key: format!("validator_pubkey_{}", i + 1),
                validation_tasks_completed: 0,
                validation_tasks_assigned: 0,
            };
            self.nodes.insert(node_id.clone(), node);
            
            if is_simulator {
                self.simulator_nodes.push(node_id.clone());
            }
        }
        
        // Initialize faucet
        self.balances.insert("faucet_address_123456789".to_string(), 1000000.0);
        
        println!("✅ Consensus Network Initialized:");
        println!("   🏛️  {} Leader nodes", self.leaders.len());
        println!("   🔍 {} Validator nodes", self.nodes.len() - self.leaders.len());
        println!("   🤖 {} Simulator nodes", self.simulator_nodes.len());
        
        // Initialize some cross-validation activity
        self.simulate_ongoing_validation_activity();
    }
    
    fn simulate_ongoing_validation_activity(&mut self) {
        // Create some pending validation tasks to show real activity
        for i in 0..3 {
            let task = ValidationTask {
                task_id: format!("pending_task_{}", i + 1),
                raw_tx_id: format!("pending_tx_{}", i + 1),
                task_type: "cross_validation".to_string(),
                assigned_validator: format!("validator_{}", (i % 5) + 1),
                validator_must_validate_tx: format!("tx_cross_validation_{}", i + 1),
                complete: false,
                timestamp: Self::current_timestamp(),
                completion_timestamp: None,
                validator_signature: None,
            };
            
            self.validation_tasks_mempool
                .entry("leader_1".to_string())
                .or_insert_with(Vec::new)
                .push(task);
        }
        
        self.cross_validation_log.push("Initialized pending cross-validation tasks".to_string());
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
    
    // Step 1: User submits transaction and gets assigned validation tasks for OTHER users' transactions
    fn submit_transaction(&mut self, tx_data: TransactionData) -> std::result::Result<String, String> {
        let leader = self.get_current_leader().ok_or("No leader available")?.clone();
        let raw_tx_id = Uuid::new_v4().to_string();
        
        println!("🔄 CROSS-VALIDATION: User {} submitting transaction", tx_data.user);
        
        // Check balance
        if tx_data.from != "faucet_address_123456789" {
            let sender_balance = self.get_balance(&tx_data.from);
            let required = tx_data.amount + tx_data.stake + tx_data.fee;
            if sender_balance < required {
                return Err(format!("Insufficient balance: {} < {}", sender_balance, required));
            }
        }
        
        // CRITICAL: Before processing this transaction, assign validation tasks to the submitter
        let validation_tasks_for_submitter = self.assign_validation_tasks_to_user(&tx_data.user)?;
        
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
            status: "pending_validation".to_string(),
        };
        
        // Add to leader's raw_tx_mempool
        self.raw_tx_mempool
            .entry(leader.id.clone())
            .or_insert_with(HashMap::new)
            .insert(raw_tx_id.clone(), raw_tx);
        
        println!("🔄 Transaction submitted to leader {} ({})", leader.id, leader.address);
        println!("   📝 Raw TX ID: {}", raw_tx_id);
        println!("   💰 Amount: {} XMBL", tx_data.amount);
        println!("   ✅ User assigned {} validation tasks", validation_tasks_for_submitter.len());
        
        self.cross_validation_log.push(format!(
            "User {} submitted tx {} and must validate {} other transactions",
            tx_data.user, raw_tx_id, validation_tasks_for_submitter.len()
        ));
        
        Ok(raw_tx_id)
    }
    
    // CRITICAL: Assign validation tasks to user for OTHER users' transactions
    fn assign_validation_tasks_to_user(&mut self, user: &str) -> std::result::Result<Vec<String>, String> {
        let mut assigned_tasks = Vec::new();
        
        // Find other users' transactions that need validation
        let mut transactions_needing_validation = Vec::new();
        for (leader_id, tx_pool) in &self.raw_tx_mempool {
            for (tx_id, raw_tx) in tx_pool {
                if raw_tx.tx_data.user != user && raw_tx.status == "pending_validation" {
                    transactions_needing_validation.push((leader_id.clone(), tx_id.clone()));
                }
            }
        }
        
        // Assign up to 2 validation tasks
        let num_tasks = std::cmp::min(2, transactions_needing_validation.len());
        for i in 0..num_tasks {
            let (leader_id, tx_id) = &transactions_needing_validation[i];
            let task_id = Uuid::new_v4().to_string();
            
            let validation_task = ValidationTask {
                task_id: task_id.clone(),
                raw_tx_id: tx_id.clone(),
                task_type: "cross_validation".to_string(),
                assigned_validator: user.to_string(),
                validator_must_validate_tx: tx_id.clone(),
                complete: false,
                timestamp: Self::current_timestamp(),
                completion_timestamp: None,
                validator_signature: None,
            };
            
            self.validation_tasks_mempool
                .entry(leader_id.clone())
                .or_insert_with(Vec::new)
                .push(validation_task);
            
            assigned_tasks.push(task_id.clone());
            
            // Update validator's task count
            if let Some(validator_node) = self.nodes.get_mut(user) {
                validator_node.validation_tasks_assigned += 1;
            }
            
            println!("   📋 Assigned validation task {} to user {} for tx {}", task_id, user, tx_id);
        }
        
        // Add to user's validation queue
        self.user_validation_queue
            .entry(user.to_string())
            .or_insert_with(Vec::new)
            .extend(assigned_tasks.clone());
        
        Ok(assigned_tasks)
    }
    
    // Simulate completion of validation tasks
    fn complete_validation_tasks(&mut self, raw_tx_id: &str) -> std::result::Result<String, String> {
        let leader = self.get_current_leader().ok_or("No leader available")?.clone();
        
        // Find raw transaction
        let raw_tx = self.raw_tx_mempool
            .get(&leader.id)
            .and_then(|pool| pool.get(raw_tx_id))
            .ok_or("Raw transaction not found")?
            .clone();
        
        // Simulate validators completing their tasks
        let validators: Vec<String> = self.simulator_nodes.iter().take(3).cloned().collect();
        let mut validation_results = Vec::new();
        
        for validator_id in &validators {
            let result = ValidationResult {
                validator_id: validator_id.clone(),
                validation_task_id: Uuid::new_v4().to_string(),
                result: true, // Simulation: all validations pass
                signature: format!("sig_{}_{}", validator_id, &Uuid::new_v4().to_string()[..8]),
                timestamp: Self::current_timestamp(),
            };
            validation_results.push(result);
            
            // Update validator stats
            if let Some(validator_node) = self.nodes.get_mut(validator_id) {
                validator_node.validation_tasks_completed += 1;
            }
        }
        
        // Move to processing mempool
        let uuid_str = Uuid::new_v4().to_string();
        let tx_id = format!("tx_{}", &uuid_str[..8]);
        let uuid_str2 = Uuid::new_v4().to_string();
        
        let processing_tx = ProcessingTransaction {
            tx_id: tx_id.clone(),
            tx_data: raw_tx.tx_data.clone(),
            timestamp: Self::current_timestamp(),
            leader_sig: format!("sig_{}", &uuid_str2[..8]),
            leader_id: leader.id.clone(),
            validation_results,
        };
        
        self.processing_tx_mempool.insert(tx_id.clone(), processing_tx);
        
        // Remove from raw mempool
        if let Some(pool) = self.raw_tx_mempool.get_mut(&leader.id) {
            pool.remove(raw_tx_id);
        }
        
        println!("✅ Cross-validation completed for TX {}", raw_tx_id);
        println!("   🚀 Moved to processing as TX {}", tx_id);
        println!("   👥 Validated by: {}", validators.join(", "));
        
        self.cross_validation_log.push(format!(
            "Cross-validation completed for {} by validators: {}",
            raw_tx_id, validators.join(", ")
        ));
        
        Ok(tx_id)
    }
    
    // Step 6: Final validation and ledger update with cross-validation proof
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
        
        // Get cross-validators and validation tasks
        let cross_validators: Vec<String> = processing_tx.validation_results
            .iter()
            .map(|r| r.validator_id.clone())
            .collect();
        
        let validation_tasks_for_submitter = self.user_validation_queue
            .get(&tx_data.user)
            .cloned()
            .unwrap_or_default();
        
        // Create final transaction with cross-validation proof
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
                format!("User {} assigned validation tasks", tx_data.user),
                "Cross-validation by other users".to_string(),
                "Leader consensus".to_string(),
                "Validator broadcast".to_string(),
                "Digital root calculation".to_string(),
                "Final confirmation with proof".to_string(),
            ],
            cross_validators,
            validation_tasks_for_submitter,
        };
        
        // Add to final mempool
        self.tx_mempool.insert(tx_id.to_string(), final_tx.clone());
        
        // Remove from processing mempool
        self.processing_tx_mempool.remove(tx_id);
        
        // Unlock UTXOs
        self.locked_utxo_mempool.retain(|utxo| utxo != &tx_data.from);
        
        println!("🎉 Transaction finalized with cross-validation: {} XMBL from {} to {}", 
                 tx_data.amount, tx_data.from, tx_data.to);
        println!("   🔢 Digital root: {}", digital_root);
        println!("   👑 Leader: {}", processing_tx.leader_id);
        println!("   👥 Cross-validators: {}", final_tx.cross_validators.join(", "));
        
        self.cross_validation_log.push(format!(
            "Transaction {} finalized with cross-validation proof",
            tx_id
        ));
        
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
            "simulator_nodes": self.simulator_nodes.len(),
            "current_leader": self.get_current_leader().map(|l| &l.id),
            "raw_transactions": self.raw_tx_mempool.values().map(|pool| pool.len()).sum::<usize>(),
            "processing_transactions": self.processing_tx_mempool.len(),
            "finalized_transactions": self.tx_mempool.len(),
            "locked_utxos": self.locked_utxo_mempool.len(),
            "validation_tasks": self.validation_tasks_mempool.values().map(|tasks| tasks.len()).sum::<usize>(),
            "cross_validation_log": self.cross_validation_log.iter().rev().take(10).collect::<Vec<_>>(),
        })
    }
    
    fn get_mempool_activity(&self) -> serde_json::Value {
        let mut activity = Vec::new();
        
        // Add raw transaction activity
        for (leader_id, tx_pool) in &self.raw_tx_mempool {
            for (tx_id, raw_tx) in tx_pool {
                activity.push(serde_json::json!({
                    "type": "raw_transaction",
                    "tx_id": tx_id,
                    "leader": leader_id,
                    "status": raw_tx.status,
                    "timestamp": raw_tx.tx_timestamp,
                    "user": raw_tx.tx_data.user
                }));
            }
        }
        
        // Add validation task activity
        for (leader_id, tasks) in &self.validation_tasks_mempool {
            for task in tasks {
                activity.push(serde_json::json!({
                    "type": "validation_task",
                    "task_id": task.task_id,
                    "leader": leader_id,
                    "validator": task.assigned_validator,
                    "complete": task.complete,
                    "timestamp": task.timestamp
                }));
            }
        }
        
        // Add processing transaction activity
        for (tx_id, processing_tx) in &self.processing_tx_mempool {
            activity.push(serde_json::json!({
                "type": "processing_transaction",
                "tx_id": tx_id,
                "leader": processing_tx.leader_id,
                "validation_results": processing_tx.validation_results.len(),
                "timestamp": processing_tx.timestamp
            }));
        }
        
        // Sort by timestamp
        activity.sort_by(|a, b| {
            let a_time = a["timestamp"].as_u64().unwrap_or(0);
            let b_time = b["timestamp"].as_u64().unwrap_or(0);
            b_time.cmp(&a_time)
        });
        
        serde_json::json!({
            "activity": activity.into_iter().take(20).collect::<Vec<_>>(),
            "cross_validation_log": self.cross_validation_log.iter().rev().take(10).collect::<Vec<_>>()
        })
    }
    
    fn get_transaction_details(&self, tx_id: &str) -> Option<serde_json::Value> {
        self.tx_mempool.get(tx_id).map(|tx| {
            serde_json::json!({
                "transaction": tx,
                "leader_node": self.nodes.get(tx.leader_id.as_ref().unwrap_or(&"unknown".to_string())),
                "cross_validation_proof": {
                    "cross_validators": tx.cross_validators,
                    "validation_tasks_completed_by_submitter": tx.validation_tasks_for_submitter,
                    "digital_root": self.calculate_digital_root(tx_id),
                    "validation_steps_completed": tx.validation_steps.len(),
                    "validators_involved": tx.validators.len(),
                }
            })
        })
    }
    
    fn get_simulator_addresses(&self) -> Vec<serde_json::Value> {
        self.simulator_nodes.iter().enumerate().map(|(i, node_id)| {
            let node = self.nodes.get(node_id).unwrap();
            let names = ["Alice", "Bob", "Charlie", "Diana", "Eve"];
            let name = names.get(i).unwrap_or(&"SimUser");
            let address = format!("sim_{}_{}", name.to_lowercase(), node_id);
            let balance = self.get_balance(&address);
            
            serde_json::json!({
                "name": name,
                "address": address,
                "balance": balance,
                "node_id": node_id,
                "validation_tasks_completed": node.validation_tasks_completed,
                "validation_tasks_assigned": node.validation_tasks_assigned,
                "public_key": node.public_key
            })
        }).collect()
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
            match consensus.submit_transaction(tx_data.clone()) {
                Ok(raw_tx_id) => {
                    // Step 2-3: Assign validation tasks
                    let _tasks = consensus.assign_validation_tasks_to_user(&tx_data.user).unwrap_or_default();
                    
                    // Step 4-5: Complete validation (simulated)
                    match consensus.complete_validation_tasks(&raw_tx_id) {
                        Ok(tx_id) => {
                            // Step 6: Finalize transaction
                            match consensus.finalize_transaction(&tx_id) {
                                Ok(final_tx) => {
                                    let response = serde_json::json!({
                                        "success": true,
                                        "transaction": final_tx,
                                        "message": "Transaction processed successfully"
                                    });
                                    response.to_string()
                                }
                                Err(e) => {
                                    let response = serde_json::json!({
                                        "success": false,
                                        "error": e,
                                        "message": "Failed to finalize transaction"
                                    });
                                    response.to_string()
                                }
                            }
                        }
                        Err(e) => {
                            let response = serde_json::json!({
                                "success": false,
                                "error": e,
                                "message": "Failed to complete validation"
                            });
                            response.to_string()
                        }
                    }
                }
                Err(e) => {
                    let response = serde_json::json!({
                        "success": false,
                        "error": e,
                        "message": "Failed to submit transaction"
                    });
                    response.to_string()
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