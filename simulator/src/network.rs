use pcl_backend::{Node, NodeRole};
use log::{info, debug, warn};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use rand::Rng;
use chrono::{DateTime, Utc};

pub struct NetworkSimulator {
    active_nodes: Arc<RwLock<HashMap<Uuid, Node>>>,
    message_history: Arc<RwLock<Vec<NetworkMessage>>>,
    leader_election_in_progress: Arc<RwLock<bool>>,
    uptime_mempool: Arc<RwLock<HashMap<String, UptimeEntry>>>,
}

#[derive(Debug, Clone)]
pub struct NetworkMessage {
    pub message_id: Uuid,
    pub from: Uuid,
    pub to: Vec<Uuid>,
    pub message_type: MessageType,
    pub timestamp: DateTime<Utc>,
    pub payload: String,
}

#[derive(Debug, Clone)]
pub enum MessageType {
    TransactionGossip,
    ValidationTask,
    LeaderElection,
    Pulse,
    PulseResponse,
    UptimeData,
    BlockchainUpdate,
    TestMessage,
}

#[derive(Debug, Clone)]
pub struct UptimeEntry {
    pub ip: String,
    pub timestamp: DateTime<Utc>,
    pub pulse_count: u64,
    pub average_response_time: f64,
}

impl NetworkSimulator {
    pub fn new(active_nodes: Arc<RwLock<HashMap<Uuid, Node>>>) -> Self {
        Self {
            active_nodes,
            message_history: Arc::new(RwLock::new(Vec::new())),
            leader_election_in_progress: Arc::new(RwLock::new(false)),
            uptime_mempool: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    pub async fn broadcast_test_message(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let active_nodes = self.active_nodes.read().await;
        let nodes: Vec<Node> = active_nodes.values().cloned().collect();
        
        if nodes.is_empty() {
            return Err("No active nodes for broadcast".into());
        }
        
        let sender_idx = rand::thread_rng().gen_range(0..nodes.len());
        let sender = &nodes[sender_idx];
        
        let message = NetworkMessage {
            message_id: Uuid::new_v4(),
            from: sender.id,
            to: nodes.iter().map(|n| n.id).collect(),
            message_type: MessageType::TestMessage,
            timestamp: Utc::now(),
            payload: "Test broadcast message".to_string(),
        };
        
        self.send_message(message).await?;
        
        debug!("Broadcast test message from node {}", sender.id);
        Ok(())
    }
    
    pub async fn trigger_leader_election(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut election_in_progress = self.leader_election_in_progress.write().await;
        if *election_in_progress {
            return Ok(()); // Election already in progress
        }
        
        *election_in_progress = true;
        info!("Starting leader election");
        
        // Simulate leader election process
        let active_nodes = self.active_nodes.read().await;
        let nodes: Vec<Node> = active_nodes.values().cloned().collect();
        
        if nodes.is_empty() {
            *election_in_progress = false;
            return Err("No nodes available for leader election".into());
        }
        
        // Phase 1: Collect uptime data
        self.collect_uptime_data(&nodes).await?;
        
        // Phase 2: Broadcast nominations
        self.broadcast_nominations(&nodes).await?;
        
        // Phase 3: Voting rounds
        self.conduct_voting_rounds(&nodes).await?;
        
        // Phase 4: Announce new leaders
        self.announce_new_leaders(&nodes).await?;
        
        *election_in_progress = false;
        info!("Leader election completed");
        Ok(())
    }
    
    async fn collect_uptime_data(&self, nodes: &[Node]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!("Collecting uptime data from {} nodes", nodes.len());
        
        for node in nodes {
            let uptime_entry = UptimeEntry {
                ip: node.ip.clone(),
                timestamp: Utc::now(),
                pulse_count: rand::thread_rng().gen_range(100..1000),
                average_response_time: rand::thread_rng().gen_range(50.0..500.0),
            };
            
            let mut uptime_mempool = self.uptime_mempool.write().await;
            uptime_mempool.insert(node.ip.clone(), uptime_entry);
        }
        
        Ok(())
    }
    
    async fn broadcast_nominations(&self, nodes: &[Node]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!("Broadcasting leader nominations");
        
        // Select top performing nodes based on uptime data
        let uptime_mempool = self.uptime_mempool.read().await;
        let mut candidates: Vec<_> = uptime_mempool.values().collect();
        
        // Sort by performance (pulse count and response time)
        candidates.sort_by(|a, b| {
            let score_a = a.pulse_count as f64 / a.average_response_time;
            let score_b = b.pulse_count as f64 / b.average_response_time;
            score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
        });
        
        // Broadcast nominations
        for node in nodes {
            let message = NetworkMessage {
                message_id: Uuid::new_v4(),
                from: node.id,
                to: nodes.iter().map(|n| n.id).collect(),
                message_type: MessageType::LeaderElection,
                timestamp: Utc::now(),
                payload: format!("Nomination: {:?}", candidates.get(0).map(|c| &c.ip)),
            };
            
            self.send_message(message).await?;
        }
        
        Ok(())
    }
    
    async fn conduct_voting_rounds(&self, nodes: &[Node]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!("Conducting voting rounds");
        
        // Simulate 3 rounds of voting
        for round in 1..=3 {
            debug!("Voting round {}", round);
            
            for node in nodes {
                // Each node votes for their preferred leader
                let vote_message = NetworkMessage {
                    message_id: Uuid::new_v4(),
                    from: node.id,
                    to: nodes.iter().map(|n| n.id).collect(),
                    message_type: MessageType::LeaderElection,
                    timestamp: Utc::now(),
                    payload: format!("Vote round {}: {}", round, node.ip),
                };
                
                self.send_message(vote_message).await?;
            }
            
            // Brief pause between rounds
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
        
        Ok(())
    }
    
    async fn announce_new_leaders(&self, nodes: &[Node]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!("Announcing new leaders");
        
        // Select leaders based on voting results (simplified)
        let leader_count = std::cmp::min(nodes.len() / 3, 5); // Up to 5 leaders
        let leaders: Vec<_> = nodes.iter().take(leader_count).collect();
        
        let announcement_message = NetworkMessage {
            message_id: Uuid::new_v4(),
            from: Uuid::new_v4(), // System message
            to: nodes.iter().map(|n| n.id).collect(),
            message_type: MessageType::LeaderElection,
            timestamp: Utc::now(),
            payload: format!("New leaders: {:?}", leaders.iter().map(|l| &l.ip).collect::<Vec<_>>()),
        };
        
        self.send_message(announcement_message).await?;
        
        info!("Announced {} new leaders", leaders.len());
        Ok(())
    }
    
    pub async fn simulate_pulse_system(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let active_nodes = self.active_nodes.read().await;
        let nodes: Vec<Node> = active_nodes.values().cloned().collect();
        
        if nodes.is_empty() {
            return Ok(());
        }
        
        // Simulate pulse messages every 20 seconds (simplified for testing)
        for node in &nodes {
            // Send pulse to family members (simplified: random 3-5 nodes)
            let family_size = rand::thread_rng().gen_range(3..=5);
            let family_members: Vec<_> = nodes
                .iter()
                .filter(|n| n.id != node.id)
                .take(family_size)
                .collect();
            
            for family_member in family_members {
                let pulse_message = NetworkMessage {
                    message_id: Uuid::new_v4(),
                    from: node.id,
                    to: vec![family_member.id],
                    message_type: MessageType::Pulse,
                    timestamp: Utc::now(),
                    payload: format!("Pulse from {}", node.ip),
                };
                
                self.send_message(pulse_message).await?;
                
                // Simulate response
                let response_time = rand::thread_rng().gen_range(10..200);
                tokio::time::sleep(tokio::time::Duration::from_millis(response_time)).await;
                
                let response_message = NetworkMessage {
                    message_id: Uuid::new_v4(),
                    from: family_member.id,
                    to: vec![node.id],
                    message_type: MessageType::PulseResponse,
                    timestamp: Utc::now(),
                    payload: format!("Pulse response from {}", family_member.ip),
                };
                
                self.send_message(response_message).await?;
            }
        }
        
        debug!("Simulated pulse system for {} nodes", nodes.len());
        Ok(())
    }
    
    pub async fn gossip_transaction(&self, transaction_id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let active_nodes = self.active_nodes.read().await;
        let leaders: Vec<Node> = active_nodes
            .values()
            .filter(|node| node.role == NodeRole::Leader)
            .cloned()
            .collect();
        
        if leaders.is_empty() {
            return Err("No leader nodes available for gossip".into());
        }
        
        // Select 3 random leaders for gossip
        let gossip_count = std::cmp::min(3, leaders.len());
        let mut gossip_targets = Vec::new();
        
        for _ in 0..gossip_count {
            let idx = rand::thread_rng().gen_range(0..leaders.len());
            gossip_targets.push(leaders[idx].clone());
        }
        
        // Send gossip messages
        for target in gossip_targets {
            let gossip_message = NetworkMessage {
                message_id: Uuid::new_v4(),
                from: target.id,
                to: leaders.iter().map(|l| l.id).collect(),
                message_type: MessageType::TransactionGossip,
                timestamp: Utc::now(),
                payload: format!("Gossip transaction: {}", transaction_id),
            };
            
            self.send_message(gossip_message).await?;
        }
        
        debug!("Gossipped transaction {} to {} leaders", transaction_id, gossip_count);
        Ok(())
    }
    
    pub async fn query_mempool_status(&self) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        // Simulate mempool query
        let mempool_size = rand::thread_rng().gen_range(0..1000);
        
        let query_message = NetworkMessage {
            message_id: Uuid::new_v4(),
            from: Uuid::new_v4(),
            to: vec![],
            message_type: MessageType::BlockchainUpdate,
            timestamp: Utc::now(),
            payload: format!("Mempool query result: {} transactions", mempool_size),
        };
        
        self.send_message(query_message).await?;
        
        debug!("Queried mempool status: {} transactions", mempool_size);
        Ok(mempool_size)
    }
    
    async fn send_message(&self, message: NetworkMessage) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut message_history = self.message_history.write().await;
        message_history.push(message);
        
        // Simulate network latency
        let latency = rand::thread_rng().gen_range(1..50);
        tokio::time::sleep(tokio::time::Duration::from_millis(latency)).await;
        
        Ok(())
    }
    
    pub async fn get_message_count(&self) -> usize {
        let message_history = self.message_history.read().await;
        message_history.len()
    }
    
    pub async fn get_uptime_stats(&self) -> HashMap<String, UptimeEntry> {
        let uptime_mempool = self.uptime_mempool.read().await;
        uptime_mempool.clone()
    }
    
    pub async fn simulate_network_partition(&self, partition_size: u32) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let active_nodes = self.active_nodes.read().await;
        let nodes: Vec<Node> = active_nodes.values().cloned().collect();
        
        if nodes.is_empty() {
            return Ok(());
        }
        
        let partition_size = std::cmp::min(partition_size as usize, nodes.len());
        
        warn!("Simulating network partition affecting {} nodes", partition_size);
        
        // Simulate dropped messages for partitioned nodes
        for i in 0..partition_size {
            let node = &nodes[i];
            
            let partition_message = NetworkMessage {
                message_id: Uuid::new_v4(),
                from: node.id,
                to: vec![],
                message_type: MessageType::TestMessage,
                timestamp: Utc::now(),
                payload: format!("Network partition: {} isolated", node.ip),
            };
            
            self.send_message(partition_message).await?;
        }
        
        Ok(())
    }
    
    pub async fn simulate_network_recovery(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Simulating network recovery");
        
        let recovery_message = NetworkMessage {
            message_id: Uuid::new_v4(),
            from: Uuid::new_v4(),
            to: vec![],
            message_type: MessageType::TestMessage,
            timestamp: Utc::now(),
            payload: "Network recovery: All nodes reconnected".to_string(),
        };
        
        self.send_message(recovery_message).await?;
        
        Ok(())
    }
    
    pub async fn clear_message_history(&self) {
        let mut message_history = self.message_history.write().await;
        message_history.clear();
    }
} 