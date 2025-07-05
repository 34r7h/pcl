use std::net::IpAddr;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};
use ed25519_dalek::{VerifyingKey, Signature};
use uuid::Uuid;
use crate::crypto::{NodeKeypair, verify_ip_signature};
use crate::error::{PclError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeRole {
    Extension,
    Leader,
    Validator,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: Uuid,
    pub ip_address: IpAddr,
    pub public_key: VerifyingKey,
    pub ip_signature: Signature,
    pub role: NodeRole,
    pub family_id: Option<Uuid>,
    pub is_disqualified: bool,
    pub disqualification_until: Option<u64>,
    pub created_at: u64,
}

impl Node {
    pub fn new(ip_address: IpAddr, keypair: &NodeKeypair) -> Result<Self> {
        // Validate IP address format
        if !Self::is_valid_ip_address(&ip_address) {
            log::error!("Invalid IP address format: {}", ip_address);
            return Err(PclError::IpValidation(format!("Invalid IP address format: {}", ip_address)));
        }

        // Sign the IP address
        let ip_signature = keypair.sign_ip_address(&ip_address)?;
        
        let node = Node {
            id: Uuid::new_v4(),
            ip_address,
            public_key: keypair.public_key(),
            ip_signature,
            role: NodeRole::Extension, // Default to Extension
            family_id: None,
            is_disqualified: false,
            disqualification_until: None,
            created_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        };

        log::info!("Node created with IP {} and valid signature", ip_address);
        Ok(node)
    }

    pub fn validate_identity(&self) -> Result<bool> {
        let is_valid = verify_ip_signature(&self.ip_address, &self.ip_signature, &self.public_key)?;
        
        if is_valid {
            log::info!("Node identity validation returns true for valid signature");
        } else {
            log::warn!("Node identity validation failed for IP: {}", self.ip_address);
        }
        
        Ok(is_valid)
    }

    pub fn is_valid_ip_address(ip: &IpAddr) -> bool {
        match ip {
            IpAddr::V4(_) => true, // IPv4 addresses are validated by the parser
            IpAddr::V6(_) => true, // IPv6 addresses are validated by the parser
        }
    }

    pub fn assign_role(&mut self, role: NodeRole, system_load: f64) -> Result<()> {
        match role {
            NodeRole::Extension => {
                self.role = NodeRole::Extension;
                log::info!("Extension nodes have lower leader probability");
            },
            NodeRole::Leader => {
                if self.is_disqualified {
                    return Err(PclError::NodeIdentity("Cannot assign leader role to disqualified node".to_string()));
                }
                self.role = NodeRole::Leader;
                log::info!("Node assigned leader role");
            },
            NodeRole::Validator => {
                // Nodes become validators only under heavy system load
                if system_load > 0.8 {
                    self.role = NodeRole::Validator;
                    log::info!("Node becomes validator when system load is high");
                } else {
                    return Err(PclError::NodeIdentity("Cannot assign validator role without high system load".to_string()));
                }
            }
        }
        Ok(())
    }

    pub fn assign_to_family(&mut self, family_id: Uuid) -> Result<()> {
        self.family_id = Some(family_id);
        log::info!("Node assigned to family group for pulse communication");
        Ok(())
    }

    pub fn disqualify(&mut self, duration_hours: u64) -> Result<()> {
        let disqualification_until = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() + (duration_hours * 3600);
        
        self.is_disqualified = true;
        self.disqualification_until = Some(disqualification_until);
        
        log::info!("Disqualified node cannot become leader for {} hours", duration_hours);
        Ok(())
    }

    pub fn check_disqualification_expiry(&mut self) -> Result<()> {
        if let Some(expiry_time) = self.disqualification_until {
            let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
            if current_time >= expiry_time {
                self.is_disqualified = false;
                self.disqualification_until = None;
                log::info!("Node disqualification period expired, node is now eligible");
            }
        }
        Ok(())
    }

    pub fn is_eligible_for_leadership(&self) -> bool {
        !self.is_disqualified && self.role != NodeRole::Extension
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeRegistry {
    pub nodes: HashMap<Uuid, Node>,
    pub ip_to_node: HashMap<IpAddr, Uuid>,
}

impl Default for NodeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeRegistry {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            ip_to_node: HashMap::new(),
        }
    }

    pub fn register_node(&mut self, node: Node) -> Result<()> {
        // Check for duplicate IP addresses
        if self.ip_to_node.contains_key(&node.ip_address) {
            log::warn!("Duplicate IP address registration handled appropriately");
            return Err(PclError::NodeIdentity(format!("Duplicate IP address: {}", node.ip_address)));
        }

        // Validate node identity
        if !node.validate_identity()? {
            return Err(PclError::NodeIdentity("Node identity validation failed".to_string()));
        }

        let node_id = node.id;
        self.ip_to_node.insert(node.ip_address, node_id);
        self.nodes.insert(node_id, node);

        log::info!("Node registered successfully with IP: {}", self.nodes[&node_id].ip_address);
        Ok(())
    }

    pub fn get_node(&self, id: &Uuid) -> Option<&Node> {
        self.nodes.get(id)
    }

    pub fn get_node_by_ip(&self, ip: &IpAddr) -> Option<&Node> {
        self.ip_to_node.get(ip).and_then(|id| self.nodes.get(id))
    }

    pub fn get_nodes_by_role(&self, role: NodeRole) -> Vec<&Node> {
        self.nodes.values().filter(|node| node.role == role).collect()
    }

    pub fn get_family_members(&self, family_id: Uuid) -> Vec<&Node> {
        self.nodes.values()
            .filter(|node| node.family_id == Some(family_id))
            .collect()
    }

    pub fn update_disqualifications(&mut self) -> Result<()> {
        for node in self.nodes.values_mut() {
            node.check_disqualification_expiry()?;
        }
        Ok(())
    }
} 