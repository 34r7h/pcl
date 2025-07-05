use pcl_backend::{Node, NodeRole, TransactionData, sign_data, hash_data};
use log::{info, debug, warn};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use rand::Rng;
use chrono::{DateTime, Utc};

pub struct TransactionGenerator {
    active_nodes: Arc<RwLock<HashMap<Uuid, Node>>>,
    transaction_counter: Arc<RwLock<u64>>,
}

impl TransactionGenerator {
    pub fn new(active_nodes: Arc<RwLock<HashMap<Uuid, Node>>>) -> Self {
        Self {
            active_nodes,
            transaction_counter: Arc::new(RwLock::new(0)),
        }
    }
    
    pub async fn generate_random_transaction(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let active_nodes = self.active_nodes.read().await;
        let nodes: Vec<Node> = active_nodes.values().cloned().collect();
        
        if nodes.len() < 3 {
            return Err("Need at least 3 nodes for transaction generation".into());
        }
        
        // Select random sender, receiver, and leader
        let sender_idx = rand::thread_rng().gen_range(0..nodes.len());
        let mut receiver_idx = rand::thread_rng().gen_range(0..nodes.len());
        while receiver_idx == sender_idx {
            receiver_idx = rand::thread_rng().gen_range(0..nodes.len());
        }
        
        let sender = &nodes[sender_idx];
        let receiver = &nodes[receiver_idx];
        
        // Find a leader node
        let leader = nodes
            .iter()
            .find(|node| node.role == NodeRole::Leader)
            .ok_or("No leader nodes available")?;
        
        // Generate transaction data based on README example
        let tx_data = self.generate_transaction_data(sender, receiver, leader).await?;
        
        // Create transaction ID
        let tx_id = self.create_transaction_id(&tx_data).await?;
        
        // Log transaction creation
        let mut counter = self.transaction_counter.write().await;
        *counter += 1;
        
        debug!("Generated transaction {}: {} -> {} (via leader {})", 
               tx_id, sender.ip, receiver.ip, leader.ip);
        
        Ok(tx_id)
    }
    
    async fn generate_transaction_data(&self, sender: &Node, receiver: &Node, leader: &Node) -> Result<TransactionData, Box<dyn std::error::Error + Send + Sync>> {
        let mut rng = rand::thread_rng();
        
        // Generate transaction amounts based on README example
        let amount = rng.gen_range(0.1..10.0); // Random amount between 0.1 and 10.0
        let fee = amount * 0.1; // 10% fee
        let stake = amount * 0.2; // 20% stake
        let total_required = amount + fee + stake;
        
        // Create UTXO data (simplified)
        let utxo_value = total_required + rng.gen_range(0.0..2.0); // Some change
        let change = utxo_value - total_required;
        
        let tx_data = TransactionData {
            to: vec![(receiver.ip.clone(), amount)],
            from: vec![(format!("{}:utxo1", sender.ip), utxo_value)],
            user: sender.ip.clone(),
            sig: None, // Will be set when signed
            stake,
            fee,
            change: Some(change),
            timestamp: Utc::now(),
            leader: Some(leader.ip.clone()),
            nonce: rng.gen::<u64>(),
        };
        
        Ok(tx_data)
    }
    
    async fn create_transaction_id(&self, tx_data: &TransactionData) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // Hash the transaction data to create ID
        let serialized = serde_json::to_string(tx_data)?;
        let hash = hash_data(serialized.as_bytes());
        Ok(hex::encode(hash))
    }
    
    pub async fn generate_burst_transactions(&self, count: u32) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let mut transaction_ids = Vec::new();
        
        for i in 0..count {
            match self.generate_random_transaction().await {
                Ok(tx_id) => {
                    transaction_ids.push(tx_id);
                    if i % 100 == 0 {
                        debug!("Generated {} transactions in burst", i);
                    }
                },
                Err(e) => {
                    warn!("Failed to generate transaction {} in burst: {}", i, e);
                }
            }
        }
        
        info!("Generated {} transactions in burst", transaction_ids.len());
        Ok(transaction_ids)
    }
    
    pub async fn generate_realistic_transaction_pattern(&self, duration_minutes: u32) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let mut transaction_ids = Vec::new();
        let start_time = std::time::Instant::now();
        let duration = std::time::Duration::from_secs(duration_minutes as u64 * 60);
        
        info!("Starting realistic transaction pattern for {} minutes", duration_minutes);
        
        // Simulate realistic transaction patterns
        // - Higher activity during "peak hours"
        // - Some transactions in clusters
        // - Occasional bursts of activity
        
        let mut minute = 0;
        while start_time.elapsed() < duration {
            // Calculate TPS based on minute of the pattern
            let tps = self.calculate_realistic_tps(minute);
            
            // Generate transactions for this minute
            let transactions_this_minute = tps * 60;
            for _ in 0..transactions_this_minute {
                match self.generate_random_transaction().await {
                    Ok(tx_id) => transaction_ids.push(tx_id),
                    Err(e) => warn!("Failed to generate transaction: {}", e),
                }
                
                // Sleep to maintain TPS
                tokio::time::sleep(std::time::Duration::from_millis(1000 / tps as u64)).await;
            }
            
            minute += 1;
            if minute % 10 == 0 {
                info!("Completed {} minutes of realistic transaction pattern", minute);
            }
        }
        
        info!("Completed realistic transaction pattern: {} transactions", transaction_ids.len());
        Ok(transaction_ids)
    }
    
    fn calculate_realistic_tps(&self, minute: u32) -> u32 {
        // Simulate realistic TPS patterns
        let base_tps = 50;
        let peak_hour_multiplier = if minute % 60 < 30 { 2 } else { 1 }; // Peak activity first half of hour
        let random_burst = if rand::thread_rng().gen_bool(0.1) { 3 } else { 1 }; // 10% chance of burst
        
        base_tps * peak_hour_multiplier * random_burst
    }
    
    pub async fn generate_stress_test_transactions(&self, max_tps: u32, duration_seconds: u32) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let mut transaction_ids = Vec::new();
        let start_time = std::time::Instant::now();
        let duration = std::time::Duration::from_secs(duration_seconds as u64);
        
        info!("Starting stress test: {} TPS for {} seconds", max_tps, duration_seconds);
        
        // Gradually increase TPS to max
        let mut current_tps = 10;
        let tps_increment = max_tps / 10; // Increase in 10 steps
        
        while start_time.elapsed() < duration {
            // Generate transactions at current TPS
            let transactions_this_second = current_tps;
            for _ in 0..transactions_this_second {
                match self.generate_random_transaction().await {
                    Ok(tx_id) => transaction_ids.push(tx_id),
                    Err(e) => warn!("Failed to generate transaction in stress test: {}", e),
                }
                
                // Sleep to maintain TPS
                tokio::time::sleep(std::time::Duration::from_millis(1000 / current_tps as u64)).await;
            }
            
            // Increase TPS every 10 seconds
            if start_time.elapsed().as_secs() % 10 == 0 && current_tps < max_tps {
                current_tps = std::cmp::min(current_tps + tps_increment, max_tps);
                info!("Increased TPS to {}", current_tps);
            }
        }
        
        info!("Completed stress test: {} transactions", transaction_ids.len());
        Ok(transaction_ids)
    }
    
    pub async fn generate_alice_bob_transaction(&self, alice_ip: &str, bob_ip: &str, leader_ip: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // Generate the specific Alice -> Bob transaction from README example
        let active_nodes = self.active_nodes.read().await;
        
        let alice = active_nodes
            .values()
            .find(|node| node.ip == alice_ip)
            .ok_or("Alice node not found")?;
        
        let bob = active_nodes
            .values()
            .find(|node| node.ip == bob_ip)
            .ok_or("Bob node not found")?;
        
        let leader = active_nodes
            .values()
            .find(|node| node.ip == leader_ip)
            .ok_or("Leader node not found")?;
        
        // Create transaction data exactly as in README
        let tx_data = TransactionData {
            to: vec![(bob.ip.clone(), 1.0)],
            from: vec![(format!("{}:utxo1", alice.ip), 2.0)],
            user: alice.ip.clone(),
            sig: None, // Will be signed later
            stake: 0.2,
            fee: 0.1,
            change: Some(0.7), // 2.0 - 1.0 - 0.2 - 0.1 = 0.7
            timestamp: Utc::now(),
            leader: Some(leader.ip.clone()),
            nonce: rand::thread_rng().gen::<u64>(),
        };
        
        let tx_id = self.create_transaction_id(&tx_data).await?;
        
        info!("Generated Alice->Bob transaction: {} ({})", tx_id, alice.ip);
        Ok(tx_id)
    }
    
    pub async fn get_transaction_count(&self) -> u64 {
        *self.transaction_counter.read().await
    }
    
    pub async fn reset_transaction_counter(&self) {
        let mut counter = self.transaction_counter.write().await;
        *counter = 0;
    }
    
    pub async fn generate_invalid_transaction(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // Generate an intentionally invalid transaction for testing
        let active_nodes = self.active_nodes.read().await;
        let nodes: Vec<Node> = active_nodes.values().cloned().collect();
        
        if nodes.is_empty() {
            return Err("No nodes available for invalid transaction".into());
        }
        
        let sender = &nodes[0];
        let receiver = &nodes[0]; // Same sender and receiver (invalid)
        
        let tx_data = TransactionData {
            to: vec![(receiver.ip.clone(), 1000.0)], // Unrealistic amount
            from: vec![(format!("{}:utxo1", sender.ip), 0.1)], // Insufficient funds
            user: sender.ip.clone(),
            sig: None,
            stake: 0.0, // No stake
            fee: 0.0,   // No fee
            change: None,
            timestamp: Utc::now(),
            leader: None, // No leader
            nonce: 0,
        };
        
        let tx_id = self.create_transaction_id(&tx_data).await?;
        warn!("Generated invalid transaction: {}", tx_id);
        Ok(tx_id)
    }
} 