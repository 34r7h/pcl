use pcl_backend::{Node, NodeKeypair, NodeRole, NodeRegistry, generate_keypair};
use log::{info, debug, warn};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use rand::Rng;

pub struct NodeSpawner {
    active_nodes: Arc<RwLock<HashMap<Uuid, Node>>>,
    node_registry: Arc<RwLock<NodeRegistry>>,
}

impl NodeSpawner {
    pub fn new(
        active_nodes: Arc<RwLock<HashMap<Uuid, Node>>>,
        node_registry: Arc<RwLock<NodeRegistry>>,
    ) -> Self {
        Self {
            active_nodes,
            node_registry,
        }
    }
    
    pub async fn spawn_extension_node(&self) -> Result<Node, Box<dyn std::error::Error + Send + Sync>> {
        let node = self.create_virtual_node(NodeRole::Extension).await?;
        
        // Register the node
        {
            let mut active_nodes = self.active_nodes.write().await;
            active_nodes.insert(node.id, node.clone());
        }
        
        {
            let mut registry = self.node_registry.write().await;
            registry.add_node(node.clone())?;
        }
        
        debug!("Spawned extension node: {} at {}", node.id, node.ip);
        Ok(node)
    }
    
    pub async fn spawn_leader_node(&self) -> Result<Node, Box<dyn std::error::Error + Send + Sync>> {
        let node = self.create_virtual_node(NodeRole::Leader).await?;
        
        // Register the node
        {
            let mut active_nodes = self.active_nodes.write().await;
            active_nodes.insert(node.id, node.clone());
        }
        
        {
            let mut registry = self.node_registry.write().await;
            registry.add_node(node.clone())?;
        }
        
        debug!("Spawned leader node: {} at {}", node.id, node.ip);
        Ok(node)
    }
    
    pub async fn spawn_validator_node(&self) -> Result<Node, Box<dyn std::error::Error + Send + Sync>> {
        let node = self.create_virtual_node(NodeRole::Validator).await?;
        
        // Register the node
        {
            let mut active_nodes = self.active_nodes.write().await;
            active_nodes.insert(node.id, node.clone());
        }
        
        {
            let mut registry = self.node_registry.write().await;
            registry.add_node(node.clone())?;
        }
        
        debug!("Spawned validator node: {} at {}", node.id, node.ip);
        Ok(node)
    }
    
    pub async fn remove_nodes(&self, count: u32) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut active_nodes = self.active_nodes.write().await;
        let mut registry = self.node_registry.write().await;
        
        let node_ids: Vec<Uuid> = active_nodes.keys().take(count as usize).copied().collect();
        
        for node_id in node_ids {
            if let Some(node) = active_nodes.remove(&node_id) {
                registry.remove_node(node_id)?;
                debug!("Removed node: {} at {}", node.id, node.ip);
            }
        }
        
        info!("Removed {} nodes", count);
        Ok(())
    }
    
    pub async fn get_random_node(&self) -> Option<Node> {
        let active_nodes = self.active_nodes.read().await;
        if active_nodes.is_empty() {
            return None;
        }
        
        let nodes: Vec<Node> = active_nodes.values().cloned().collect();
        let idx = rand::thread_rng().gen_range(0..nodes.len());
        Some(nodes[idx].clone())
    }
    
    pub async fn get_random_leader(&self) -> Option<Node> {
        let active_nodes = self.active_nodes.read().await;
        let leaders: Vec<Node> = active_nodes
            .values()
            .filter(|node| node.role == NodeRole::Leader)
            .cloned()
            .collect();
        
        if leaders.is_empty() {
            return None;
        }
        
        let idx = rand::thread_rng().gen_range(0..leaders.len());
        Some(leaders[idx].clone())
    }
    
    pub async fn get_all_leaders(&self) -> Vec<Node> {
        let active_nodes = self.active_nodes.read().await;
        active_nodes
            .values()
            .filter(|node| node.role == NodeRole::Leader)
            .cloned()
            .collect()
    }
    
    pub async fn get_node_count(&self) -> u32 {
        let active_nodes = self.active_nodes.read().await;
        active_nodes.len() as u32
    }
    
    pub async fn get_leader_count(&self) -> u32 {
        let active_nodes = self.active_nodes.read().await;
        active_nodes
            .values()
            .filter(|node| node.role == NodeRole::Leader)
            .count() as u32
    }
    
    async fn create_virtual_node(&self, role: NodeRole) -> Result<Node, Box<dyn std::error::Error + Send + Sync>> {
        // Generate a virtual IP address
        let ip = self.generate_virtual_ip().await;
        
        // Generate keypair
        let keypair = generate_keypair();
        
        // Create node using the new constructor
        let node = Node::new_with_string_ip(ip, keypair, role)?;
        
        debug!("Created virtual node: {} ({:?}) at {}", node.id, node.role, node.ip);
        Ok(node)
    }
    
    async fn generate_virtual_ip(&self) -> String {
        // Generate a realistic-looking IP address for simulation
        // Use 192.168.x.x range for virtual nodes
        let mut rng = rand::thread_rng();
        let a = 192;
        let b = 168;
        let c = rng.gen_range(1..255);
        let d = rng.gen_range(1..255);
        
        format!("{}.{}.{}.{}", a, b, c, d)
    }
    
    pub async fn simulate_node_failure(&self, node_id: Uuid) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut active_nodes = self.active_nodes.write().await;
        let mut registry = self.node_registry.write().await;
        
        if let Some(node) = active_nodes.remove(&node_id) {
            registry.remove_node(node_id)?;
            warn!("Simulated node failure: {} at {}", node.id, node.ip);
        }
        
        Ok(())
    }
    
    pub async fn simulate_node_recovery(&self, _node_id: Uuid) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // For now, just spawn a new node with the same role
        let role = NodeRole::Extension; // Default role for recovery
        let recovered_node = self.create_virtual_node(role).await?;
        
        {
            let mut active_nodes = self.active_nodes.write().await;
            active_nodes.insert(recovered_node.id, recovered_node.clone());
        }
        
        {
            let mut registry = self.node_registry.write().await;
            registry.add_node(recovered_node.clone())?;
        }
        
        info!("Simulated node recovery: {} at {}", recovered_node.id, recovered_node.ip);
        Ok(())
    }
    
    pub async fn simulate_network_partition(&self, partition_size: u32) -> Result<Vec<Node>, Box<dyn std::error::Error + Send + Sync>> {
        let active_nodes = self.active_nodes.read().await;
        let all_nodes: Vec<Node> = active_nodes.values().cloned().collect();
        
        if all_nodes.is_empty() {
            return Ok(Vec::new());
        }
        
        let partition_size = std::cmp::min(partition_size as usize, all_nodes.len());
        let mut partitioned_nodes = Vec::new();
        
        for _i in 0..partition_size {
            let idx = rand::thread_rng().gen_range(0..all_nodes.len());
            partitioned_nodes.push(all_nodes[idx].clone());
        }
        
        warn!("Simulated network partition affecting {} nodes", partitioned_nodes.len());
        Ok(partitioned_nodes)
    }
    
    pub async fn promote_to_leader(&self, node_id: Uuid) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut active_nodes = self.active_nodes.write().await;
        let mut registry = self.node_registry.write().await;
        
        if let Some(node) = active_nodes.get_mut(&node_id) {
            node.role = NodeRole::Leader;
            registry.update_node_role(node_id, NodeRole::Leader)?;
            info!("Promoted node {} to leader", node_id);
        }
        
        Ok(())
    }
    
    pub async fn demote_from_leader(&self, node_id: Uuid) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut active_nodes = self.active_nodes.write().await;
        let mut registry = self.node_registry.write().await;
        
        if let Some(node) = active_nodes.get_mut(&node_id) {
            node.role = NodeRole::Extension;
            registry.update_node_role(node_id, NodeRole::Extension)?;
            info!("Demoted node {} from leader", node_id);
        }
        
        Ok(())
    }
} 