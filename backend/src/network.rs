// Network module - libp2p integration for PCL

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, mpsc};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use futures::StreamExt;

use libp2p::{
    gossipsub::{self, IdentTopic as Topic, MessageId, PublishError},
    identity,
    mdns,
    noise, ping, identify, Swarm, PeerId,
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux, Multiaddr, Transport,
};

use crate::error::{PclError, Result};
use crate::node::{Node, NodeRole}; // Node might not be directly used here anymore, PeerId is key
use crate::transaction::{RawTransaction, ValidationTask};


// Network message types (remains the same, but will be serialized for libp2p)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetworkMessage {
    TransactionGossip(TransactionGossipMessage),
    ValidationTask(ValidationTaskMessage),
    LeaderElectionVote(LeaderElectionMessage), // Renamed for clarity
    CandidateProfile(CandidateProfileMessage), // New message for broadcasting candidate info
    Pulse(PulseMessage),
    PulseResponse(PulseResponseMessage),
    UptimeData(UptimeMessage),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateProfileMessage {
    pub election_id: String,
    pub candidate_node_uuid: String, // Application Node UUID
    pub performance_score: f64,
    pub uptime_score: f64,
    pub timestamp: DateTime<Utc>,
}

// Define Gossipsub topics
const GENERAL_TOPIC_STR: &str = "pcl/general";
const TX_GOSSIP_TOPIC_STR: &str = "pcl/tx/gossip";
const LEADER_ELECTION_TOPIC_STR: &str = "pcl/leader_election";
// For "direct" messages via topic (less ideal but simpler for now)
fn validation_task_topic(node_id_str: &str) -> Topic {
    Topic::new(format!("pcl/validation_task/{}", node_id_str))
}
fn pulse_topic(family_id_str: &str) -> Topic {
    Topic::new(format!("pcl/pulse/{}", family_id_str))
}
fn pulse_response_topic(node_id_str: &str) -> Topic {
    Topic::new(format!("pcl/pulse_response/{}", node_id_str))
}


#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "PclNetworkEvent")]
pub struct PclNetworkBehaviour {
    pub gossipsub: gossipsub::Behaviour,
    pub mdns: mdns::tokio::Behaviour,
    pub identify: identify::Behaviour,
    pub ping: ping::Behaviour,
}

// Events emitted by PclNetworkBehaviour to be handled by the Swarm owner
#[derive(Debug)]
pub enum PclNetworkEvent {
    Gossipsub(gossipsub::Event),
    Mdns(mdns::Event),
    Identify(identify::Event),
    Ping(ping::Event),
}

impl From<gossipsub::Event> for PclNetworkEvent {
    fn from(event: gossipsub::Event) -> Self {
        PclNetworkEvent::Gossipsub(event)
    }
}

impl From<mdns::Event> for PclNetworkEvent {
    fn from(event: mdns::Event) -> Self {
        PclNetworkEvent::Mdns(event)
    }
}

impl From<identify::Event> for PclNetworkEvent {
    fn from(event: identify::Event) -> Self {
        PclNetworkEvent::Identify(event)
    }
}

impl From<ping::Event> for PclNetworkEvent {
    fn from(event: ping::Event) -> Self {
        PclNetworkEvent::Ping(event)
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionGossipMessage {
    pub tx_id: String,
    pub raw_transaction: RawTransaction,
    pub leader_id: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationTaskMessage {
    pub task_id: String,
    pub task: ValidationTask,
    pub target_node: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderElectionMessage {
    pub election_id: String,
    pub candidate_id: String,
    pub votes: u64,
    pub round: u8,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PulseMessage {
    pub pulse_id: String,
    pub sender_peer_id: String, // libp2p PeerId of the sender
    pub sender_node_uuid: String, // Application-level Node UUID of the sender
    pub family_id: Uuid,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PulseResponseMessage {
    pub pulse_id: String,
    pub responder_peer_id: String, // libp2p PeerId of the responder
    pub responder_node_uuid: String, // Application-level Node UUID of the responder
    pub response_time_ms: u64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UptimeMessage {
    pub node_id: String,
    pub uptime_percentage: f64,
    pub last_seen: DateTime<Utc>,
    pub pulse_count: u64,
}

// Network manager for handling P2P communication using libp2p
pub struct NetworkManager {
    pub swarm: Swarm<PclNetworkBehaviour>,
    // Channel to send received messages to the ConsensusManager or other handler
    pub message_sender: mpsc::Sender<NetworkMessage>,
    // Keep track of local peer_id for identify purposes if needed
    pub local_peer_id: PeerId,
}


impl NetworkManager {
    pub async fn new(message_sender: mpsc::Sender<NetworkMessage>) -> Result<Self> {
        // Create a random PeerId
        let local_key = identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(local_key.public());
        log::info!("Local peer ID: {:?}", local_peer_id);

        // Set up an encrypted DNS-enabled TCP Transport
        let transport = tcp::tokio::Transport::new(tcp::Config::default().nodelay(true))
            .upgrade(libp2p::core::upgrade::Version::V1Lazy)
            .authenticate(noise::Config::new(&local_key)?)
            .multiplex(yamux::Config::default())
            .timeout(std::time::Duration::from_secs(20))
            .boxed();

        // Create Gossipsub configuration
        let gossipsub_config = gossipsub::ConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(10))
            .validation_mode(gossipsub::ValidationMode::Strict) // Non-strict if messages are pre-validated
            .message_id_fn(|message: &gossipsub::Message| {
                // Generate a message ID, e.g., by hashing contents
                let mut s = std::collections::hash_map::DefaultHasher::new();
                message.data.hash(&mut s);
                MessageId::from(std::hash::Hasher::finish(&s).to_string())
            })
            .build()
            .map_err(|e| PclError::NetworkInitialization(e.to_string()))?;

        let gossipsub = gossipsub::Behaviour::new(
            gossipsub::MessageAuthenticity::Signed(local_key.clone()), // Or Anonymous if not signing gossip messages
            gossipsub_config,
        )?;

        // Create mDNS
        let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), local_peer_id)?;

        // Create Identify
        let identify_config = identify::Config::new("pcl/1.0.0".to_string(), local_key.public())
            .with_agent_version(format!("pcl-node/{}", env!("CARGO_PKG_VERSION")));
        let identify = identify::Behaviour::new(identify_config);

        // Create Ping
        let ping = ping::Behaviour::new(ping::Config::new());

        // Create the PclNetworkBehaviour
        let behaviour = PclNetworkBehaviour {
            gossipsub,
            mdns,
            identify,
            ping,
        };

        // Create the Swarm
        let swarm = Swarm::with_tokio_executor(transport, behaviour, local_peer_id);

        let mut network_manager = NetworkManager {
            swarm,
            message_sender,
            local_peer_id,
        };

        // Subscribe to initial topics
        let general_topic = Topic::new(GENERAL_TOPIC_STR);
        network_manager.swarm.behaviour_mut().gossipsub.subscribe(&general_topic)
            .map_err(|e| PclError::NetworkError(format!("Failed to subscribe to general topic: {}", e)))?;

        let tx_topic = Topic::new(TX_GOSSIP_TOPIC_STR);
         network_manager.swarm.behaviour_mut().gossipsub.subscribe(&tx_topic)
            .map_err(|e| PclError::NetworkError(format!("Failed to subscribe to tx topic: {}", e)))?;

        let leader_election_topic = Topic::new(LEADER_ELECTION_TOPIC_STR);
        network_manager.swarm.behaviour_mut().gossipsub.subscribe(&leader_election_topic)
            .map_err(|e| PclError::NetworkError(format!("Failed to subscribe to leader election topic: {}", e)))?;
        
        // Subscribe to own "direct" topics (for validation tasks, pulses targeted at self)
        // This requires knowing the node's own ID string. For now, use local_peer_id.to_string()
        let self_validation_topic = validation_task_topic(&local_peer_id.to_string());
        network_manager.swarm.behaviour_mut().gossipsub.subscribe(&self_validation_topic)
            .map_err(|e| PclError::NetworkError(format!("Failed to subscribe to self validation topic: {}", e)))?;
        
        let self_pulse_response_topic = pulse_response_topic(&local_peer_id.to_string());
        network_manager.swarm.behaviour_mut().gossipsub.subscribe(&self_pulse_response_topic)
            .map_err(|e| PclError::NetworkError(format!("Failed to subscribe to self pulse response topic: {}", e)))?;


        log::info!("NetworkManager (libp2p) created. Local Peer ID: {}", local_peer_id);
        Ok(network_manager)
    }

    pub async fn start_listening(&mut self, listen_addr_str: &str) -> Result<()> {
        let listen_addr: Multiaddr = listen_addr_str.parse()
            .map_err(|e| PclError::NetworkInitialization(format!("Invalid listen address {}: {}", listen_addr_str, e)))?;
        self.swarm.listen_on(listen_addr.clone())
            .map_err(|e| PclError::NetworkInitialization(format!("Failed to listen on {}: {}", listen_addr_str, e)))?;
        log::info!("Network listening on {}", listen_addr);
        Ok(())
    }

    // Helper to publish a NetworkMessage to a specific topic
    async fn publish_message(&mut self, topic: &Topic, message: NetworkMessage) -> Result<(), PublishError> {
        let message_bytes = serde_json::to_vec(&message)
            .map_err(|e| {
                log::error!("Failed to serialize message for publish: {}", e);
                // This conversion is not ideal, PublishError is specific.
                // Consider a custom error type or logging and returning a generic PublishError.
                PublishError::Generic("Serialization failed".to_string())
            })?;
        self.swarm.behaviour_mut().gossipsub.publish(topic.clone(), message_bytes)
            .map(|_| ())
    }

    pub async fn gossip_transaction(&mut self, local_node_id: String, raw_tx: &RawTransaction) -> Result<()> {
        let topic = Topic::new(TX_GOSSIP_TOPIC_STR);
        let message = NetworkMessage::TransactionGossip(TransactionGossipMessage {
            tx_id: raw_tx.raw_tx_id.clone(),
            raw_transaction: raw_tx.clone(),
            leader_id: local_node_id, // This should be the PeerId string of the leader
            timestamp: Utc::now(),
        });
        self.publish_message(&topic, message).await
            .map_err(|e| PclError::NetworkError(format!("Failed to gossip transaction: {}", e)))
    }

    pub async fn send_validation_task(&mut self, task: &ValidationTask, target_node_peer_id_str: &str) -> Result<()> {
        // Using a specific topic for the target node as a form of "direct" messaging
        let topic = validation_task_topic(target_node_peer_id_str);
         // Ensure this node subscribes to its own validation_task_topic if it's also a target
        if target_node_peer_id_str == self.local_peer_id.to_string() {
            if !self.swarm.behaviour().gossipsub.topics().any(|t| t == &topic.hash()){
                 self.swarm.behaviour_mut().gossipsub.subscribe(&topic).map_err(|e| PclError::NetworkError(format!("Failed to subscribe to validation task topic {}: {}", topic.hash(), e)))?;
            }
        }

        let message = NetworkMessage::ValidationTask(ValidationTaskMessage {
            task_id: task.task_id.clone(),
            task: task.clone(),
            target_node: target_node_peer_id_str.to_string(),
            timestamp: Utc::now(),
        });
        self.publish_message(&topic, message).await
            .map_err(|e| PclError::NetworkError(format!("Failed to send validation task: {}", e)))
    }

    // local_node_uuid is the application-level UUID of the sending node.
    pub async fn send_pulse(&mut self, local_node_uuid: String, family_id: Uuid) -> Result<()> {
        let topic = pulse_topic(&family_id.to_string());
        let message = NetworkMessage::Pulse(PulseMessage {
            pulse_id: Uuid::new_v4().to_string(),
            sender_peer_id: self.local_peer_id.to_string(), // libp2p PeerId
            sender_node_uuid, // Application Node UUID
            family_id,
            timestamp: Utc::now(),
        });
        self.publish_message(&topic, message).await
            .map_err(|e| PclError::NetworkError(format!("Failed to send pulse: {}", e)))
    }

    // local_node_uuid is the application-level UUID of this responding node.
    // target_node_peer_id_str is the libp2p PeerId of the original pulse sender (who we are responding to).
    pub async fn send_pulse_response(&mut self, local_node_uuid: String, target_node_peer_id_str: &str, pulse_id: &str, response_time_ms: u64) -> Result<()> {
        let topic = pulse_response_topic(target_node_peer_id_str);
        // Ensure subscription if responding to self (for testing or specific scenarios)
        if target_node_peer_id_str == self.local_peer_id.to_string() {
            if !self.swarm.behaviour().gossipsub.topics().any(|t| t == &topic.hash()){
                 self.swarm.behaviour_mut().gossipsub.subscribe(&topic).map_err(|e| PclError::NetworkError(format!("Failed to subscribe to self pulse response topic {}: {}", topic.hash(), e)))?;
            }
        }
        let message = NetworkMessage::PulseResponse(PulseResponseMessage {
            pulse_id: pulse_id.to_string(),
            responder_peer_id: self.local_peer_id.to_string(), // libp2p PeerId
            responder_node_uuid, // Application Node UUID
            response_time_ms,
            timestamp: Utc::now(),
        });
        self.publish_message(&topic, message).await
            .map_err(|e| PclError::NetworkError(format!("Failed to send pulse response: {}", e)))
    }

    pub async fn broadcast_leader_election(&mut self, election_id: &str, candidate_id: &str, votes: u64, round: u8) -> Result<()> {
        let topic = Topic::new(LEADER_ELECTION_TOPIC_STR);
        let message = NetworkMessage::LeaderElection(LeaderElectionMessage {
            election_id: election_id.to_string(),
            candidate_id: candidate_id.to_string(), // Should be PeerId string
            votes,
            round,
            timestamp: Utc::now(),
        });
        self.publish_message(&topic, message).await
            .map_err(|e| PclError::NetworkError(format!("Failed to broadcast leader election: {}", e)))
    }

    pub async fn broadcast_candidate_profile(&mut self, election_id: String, candidate_node_uuid: String, performance_score: f64, uptime_score: f64) -> Result<()> {
        let topic = Topic::new(LEADER_ELECTION_TOPIC_STR); // Or a new topic like "pcl/candidate_profiles"
        let message = NetworkMessage::CandidateProfile(CandidateProfileMessage {
            election_id,
            candidate_node_uuid,
            performance_score,
            uptime_score,
            timestamp: Utc::now(),
        });
        self.publish_message(&topic, message).await
            .map_err(|e| PclError::NetworkError(format!("Failed to broadcast candidate profile: {}", e)))
    }

    pub async fn broadcast_uptime_data(&mut self, local_node_id: String, uptime_percentage: f64, pulse_count: u64) -> Result<()> {
        let topic = Topic::new(GENERAL_TOPIC_STR); // Or a specific uptime topic
        let message = NetworkMessage::UptimeData(UptimeMessage {
            node_id: local_node_id, // PeerId string
            uptime_percentage,
            last_seen: Utc::now(),
            pulse_count,
        });
        self.publish_message(&topic, message).await
            .map_err(|e| PclError::NetworkError(format!("Failed to broadcast uptime data: {}", e)))
    }

    pub async fn add_explicit_peer(&mut self, peer_id_str: &str, addr_str: &str) -> Result<()> {
        let peer_id: PeerId = peer_id_str.parse().map_err(|e| PclError::InvalidData(format!("Invalid peer ID {}: {}", peer_id_str, e)))?;
        let addr: Multiaddr = addr_str.parse().map_err(|e| PclError::InvalidData(format!("Invalid multiaddress {}: {}", addr_str, e)))?;
        self.swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
        // For Kademlia, you would use: self.swarm.behaviour_mut().kademlia.add_address(&peer_id, addr);
        // For mDNS, discovery is automatic. For Identify, it happens on connection.
        // For direct dialing:
        self.swarm.dial(addr.clone())
            .map_err(|e| PclError::NetworkError(format!("Failed to dial {}: {:?}", addr, e)))?;
        log::info!("Attempting to dial peer {} at {}", peer_id_str, addr_str);
        Ok(())
    }


    // This method should be run in a loop by the application's main async runtime
    pub async fn run_event_loop(&mut self) {
        loop {
            match self.swarm.select_next_some().await {
                SwarmEvent::Behaviour(PclNetworkEvent::Mdns(event)) => match event {
                    mdns::Event::Discovered(list) => {
                        for (peer_id, multiaddr) in list {
                            log::info!("mDNS discovered a new peer: {} at {}", peer_id, multiaddr);
                            self.swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                            // Could also dial them: self.swarm.dial(multiaddr);
                        }
                    }
                    mdns::Event::Expired(list) => {
                        for (peer_id, _multiaddr) in list {
                            log::info!("mDNS discover_peer_id {} expired", peer_id);
                            self.swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
                        }
                    }
                },
                SwarmEvent::Behaviour(PclNetworkEvent::Identify(event)) => match event {
                    identify::Event::Received { peer_id, info } => {
                        log::info!("Identify Received from {}: {:?}", peer_id, info);
                        // info.listen_addrs can be used to add addresses to routing table (e.g. Kademlia)
                        for addr in info.listen_addrs {
                             self.swarm.add_address(peer_id, addr.clone());
                        }
                    }
                    identify::Event::Sent { peer_id } => {
                        log::debug!("Identify Sent to {}", peer_id);
                    }
                    identify::Event::Pushed { peer_id, info: _ } => {
                         log::debug!("Identify Pushed for {}", peer_id);
                    }
                    identify::Event::Error { peer_id, error } => {
                        log::error!("Identify Error with {}: {:?}", peer_id, error);
                    }
                },
                SwarmEvent::Behaviour(PclNetworkEvent::Gossipsub(event)) => match event {
                    gossipsub::Event::Message {
                        propagation_source: _peer_id,
                        message_id: _id,
                        message,
                    } => {
                        match serde_json::from_slice::<NetworkMessage>(&message.data) {
                            Ok(network_msg) => {
                                log::debug!("Received Gossipsub message: {:?}", network_msg);
                                if let Err(e) = self.message_sender.send(network_msg).await {
                                    log::error!("Error sending message to handler: {}", e);
                                }
                            }
                            Err(e) => {
                                log::error!("Failed to deserialize Gossipsub message: {}", e);
                            }
                        }
                    }
                    gossipsub::Event::Subscribed { peer_id, topic } => {
                        log::info!("Peer {} subscribed to topic '{}'", peer_id, topic);
                    }
                    gossipsub::Event::Unsubscribed { peer_id, topic } => {
                        log::info!("Peer {} unsubscribed from topic '{}'", peer_id, topic);
                    }
                    gossipsub::Event::GossipsubNotSupported { peer_id } => {
                         log::warn!("Peer {} does not support Gossipsub", peer_id);
                    }
                },
                 SwarmEvent::Behaviour(PclNetworkEvent::Ping(event)) => match event {
                    ping::Event { peer, result } => match result {
                        Ok(rtt) => log::debug!("Ping to {} is {:?}", peer, rtt),
                        Err(e) => log::warn!("Ping to {} failed: {:?}", peer, e),
                    }
                },
                SwarmEvent::NewListenAddr { address, .. } => {
                    log::info!("Local node listening on: {:?}", address);
                }
                SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                    log::info!("Connected to {}: {:?}", peer_id, endpoint.get_remote_address());
                    // It's good practice to add them to Gossipsub's explicit peers if not already via discovery
                    self.swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                }
                SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                    log::info!("Connection to {} closed: {:?}", peer_id, cause);
                    self.swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
                }
                SwarmEvent::IncomingConnection { local_addr, send_back_addr } => {
                    log::info!("Incoming connection from {} to {}", send_back_addr, local_addr);
                }
                SwarmEvent::IncomingConnectionError { local_addr, send_back_addr, error } => {
                    log::warn!("Incoming connection error from {} to {}: {:?}", send_back_addr, local_addr, error);
                }
                SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                    log::warn!("Outgoing connection error to {:?}: {:?}", peer_id, error);
                }
                SwarmEvent::Dialing { peer_id, .. } => {
                     log::debug!("Dialing {:?}", peer_id);
                }
                _ => {
                    // log::trace!("Unhandled Swarm Event: {:?}", event);
                }
            }
        }
    }

    // Utility methods (can be expanded)
    pub fn local_peer_id_str(&self) -> String {
        self.local_peer_id.to_string()
    }

    pub fn get_connected_peers(&self) -> Vec<PeerId> {
        self.swarm.connected_peers().cloned().collect()
    }

    pub fn get_network_stats(&self) -> NetworkStats {
        let connected_peers = self.swarm.network_info().num_peers();
        // messages_sent is harder to track directly without custom logic in publish
        // For now, set to 0 or approximate based on topic subscriptions / known broadcasts
        NetworkStats {
            connected_peers,
            messages_sent: 0, // Placeholder
            // Uptime and health would need more sophisticated tracking
            uptime_percentage: 100.0, // Placeholder
            network_health: if connected_peers > 0 { 100.0 } else { 0.0 }, // Basic health
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStats {
    pub connected_peers: usize,
    pub messages_sent: usize, // This will be harder to track accurately with libp2p directly
    pub uptime_percentage: f64,
    pub network_health: f64,
}

// The old run_network_loop is replaced by NetworkManager::run_event_loop
// which needs to be spawned by the main application.