use libp2p::{
    core::upgrade,
    futures::StreamExt,
    gossipsub::{
        self, Gossipsub, GossipsubEvent, GossipsubMessage, IdentTopic as Topic, MessageAuthenticity,
        ValidationMode, MessageId,
    },
    identity,
    mdns::{Mdns, MdnsEvent},
    noise,
    swarm::{NetworkBehaviourEventProcess, Swarm, SwarmBuilder, SwarmEvent, behaviour::toggle::Toggle},
    tcp::{GenTcpConfig, TokioTcpTransport},
    yamux, Multiaddr, PeerId, Transport,
};
use tokio::{sync::mpsc, select};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::Duration;
use serde::{Serialize, Deserialize};
use log::{info, error, warn};

use crate::data_structures::{TransactionData, RawTxId, NodeId, UptimeEntry, RawTransactionEntry, ProcessingTransactionEntry, ValidationTaskItem}; // Assuming these are needed for messages

// Define the types of messages that can be sent over the network
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConsensusMessage {
    // Transaction related messages
    RawTransactionShare { // Gossiped by leaders when a new raw tx is received (README Step 2)
        from_node_id: NodeId,
        raw_tx_id: RawTxId,
        raw_tx_entry: RawTransactionEntry, // Contains tx_data
    },
    ValidationTaskRequest { // Sent from leader to user (Alice) (README Step 3)
        // This might be an off-chain communication in reality, or a direct message if Alice is a node.
        // For now, assume it's a network message if Alice is also a libp2p node.
        // If Alice is an extension, this would be an API call.
        // Let's assume for inter-node (leader-validator) communication for now.
        tasks: Vec<ValidationTaskItem>, // Tasks for a validator to complete for a raw_tx_id
        raw_tx_id: RawTxId,
    },
    ValidationTaskSubmission { // Sent from user (Alice) to leaders (README Step 4)
        from_user_or_validator_id: NodeId, // Alice's NodeId or Validator's NodeId
        raw_tx_id: RawTxId,
        completed_tasks: Vec<ValidationTaskItem>, // Tasks marked complete with signatures
    },
    ProcessingTransactionShare { // Gossiped by leaders after validation and averaging (README Step 5 & 6)
        from_node_id: NodeId,
        tx_id: String, // New tx_id (hash of avg_ts:tx_data)
        processing_tx_entry: ProcessingTransactionEntry,
    },
    InvalidateTransaction { // Gossiped on any invalidation (README end of workflow)
        raw_tx_id: Option<RawTxId>, // Can be raw_tx_id or final_tx_id
        tx_id: Option<String>,
        reason: String,
    },

    // Leader election related messages
    UptimePulse { // Sent every 20 seconds (README "A leader is born")
        from_node_id: NodeId,
        timestamp: i64, // For calculating response time
    },
    UptimePulseResponse { // Sent back by receiving node
        from_node_id: NodeId,
        original_timestamp: i64, // To match with the pulse
        response_timestamp: i64,
    },
    UptimeDataBroadcast { // Sent every 2 hours
        from_node_id: NodeId,
        uptime_mempool_snapshot: Vec<(NodeId, UptimeEntry)>, // Or the full map if small enough
    },
    LeaderVote { // Sent during run-off voting
        from_node_id: NodeId,
        round: u32,
        voted_for_node_ids: Vec<NodeId>,
    },
    NewLeaderList { // Broadcast after voting is complete
        leader_list_hash: String,
        sorted_leader_ids: Vec<NodeId>,
    },
    // TODO: Add more specific messages as needed, e.g., for DLT finality steps
}

// Create a custom network behaviour that combines Gossipsub and Mdns
#[derive(libp2p::NetworkBehaviour)]
#[behaviour(event_process = true)]
pub struct ConsensusBehaviour {
    pub gossipsub: Gossipsub,
    pub mdns: Toggle<Mdns>, // Use Toggle to enable/disable mDNS
    #[behaviour(ignore)]
    pub app_message_sender: mpsc::UnboundedSender<ConsensusMessage>, // To send received messages to the application logic (e.g. ConsensusNode)
    #[behaviour(ignore)]
    pub local_peer_id: PeerId,
}

impl NetworkBehaviourEventProcess<GossipsubEvent> for ConsensusBehaviour {
    fn inject_event(&mut self, event: GossipsubEvent) {
        if let GossipsubEvent::Message {
            propagation_source: _peer_id, // The peer who sent us the message
            message_id: _id,           // The ID of the message
            message,                   // The GossipsubMessage
        } = event
        {
            match serde_json::from_slice::<ConsensusMessage>(&message.data) {
                Ok(consensus_msg) => {
                    // Forward the deserialized message to the application logic (e.g., ConsensusNode)
                    info!("Gossipsub: Received consensus message from {:?}, forwarding to app logic.", message.source);
                    if let Err(e) = self.app_message_sender.send(consensus_msg) {
                        error!("Gossipsub: Error sending message to app logic: {}", e);
                    }
                }
                Err(e) => {
                    warn!("Gossipsub: Failed to deserialize message from {:?}: {}", message.source, e);
                }
            }
        }
        // Handle other GossipsubEvents like Subscribed, Unsubscribed, Ggossipped if needed
    }
}

impl NetworkBehaviourEventProcess<MdnsEvent> for ConsensusBehaviour {
    fn inject_event(&mut self, event: MdnsEvent) {
        match event {
            MdnsEvent::Discovered(list) => {
                for (peer_id, multiaddr) in list {
                    info!("mDNS: Discovered new peer: {} at {}", peer_id, multiaddr);
                    self.gossipsub.add_explicit_peer(&peer_id);
                }
            }
            MdnsEvent::Expired(list) => {
                for (peer_id, multiaddr) in list {
                    info!("mDNS: Peer expired: {} at {}", peer_id, multiaddr);
                    // Consider removing the peer from Gossipsub's explicit peers if necessary,
                    // though Gossipsub has its own mechanisms for handling unresponsive peers.
                    // self.gossipsub.remove_explicit_peer(&peer_id);
                }
            }
        }
    }
}

pub struct NetworkManager {
    pub swarm: Swarm<ConsensusBehaviour>,
    // message_receiver is removed, as messages are sent directly to ConsensusNode via channel
    consensus_topic: Topic,
}

impl NetworkManager {
    // Updated signature: `app_message_sender` is for NetworkManager to send *to* the app (ConsensusNode)
    pub async fn new(enable_mdns: bool, app_message_sender: mpsc::UnboundedSender<ConsensusMessage>) -> Result<Self, Box<dyn std::error::Error>> {
        let local_key = identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(local_key.public());
        info!("Local peer id: {}", local_peer_id);

        let transport = TokioTcpTransport::new(GenTcpConfig::default().nodelay(true))
            .upgrade(upgrade::Version::V1)
            .authenticate(noise::NoiseAuthenticated::xx(&local_key)?)
            .multiplex(yamux::YamuxConfig::default())
            .boxed();

        // Create a Gossipsub topic
        let consensus_topic = Topic::new("consensus-messages");

        // Create a Gossipsub behaviour
        let gossipsub_config = gossipsub::GossipsubConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(10)) // TODO: Configure appropriately
            .validation_mode(ValidationMode::Strict) // Enforce message signing (though not fully implemented here yet)
            // .message_id_fn(|message: &GossipsubMessage| { // Example of custom message ID
            //     let mut s = DefaultHasher::new();
            //     message.data.hash(&mut s);
            //     MessageId::from(s.finish().to_string())
            // })
            .build()?;

        let mut gossipsub = Gossipsub::new(MessageAuthenticity::Signed(local_key.clone()), gossipsub_config)?;
        gossipsub.subscribe(&consensus_topic)?;

        let mdns_behaviour = if enable_mdns {
            Some(Mdns::new(Default::default()).await?)
        } else {
            None
        };

        let behaviour = ConsensusBehaviour {
            gossipsub,
            mdns: mdns_behaviour.into(),
            app_message_sender, // Use the passed-in sender
            local_peer_id,
        };

        let mut swarm = SwarmBuilder::with_tokio_executor(transport, behaviour, local_peer_id).build();
        swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

        Ok(NetworkManager {
            swarm,
            // message_receiver is removed
            consensus_topic,
        })
    }

    pub fn publish_message(&mut self, message: &ConsensusMessage) -> Result<(), String> {
        let serialized_message = serde_json::to_vec(message).map_err(|e| e.to_string())?;
        self.swarm
            .behaviour_mut()
            .gossipsub
            .publish(self.consensus_topic.clone(), serialized_message)
            .map(|_| ())
            .map_err(|e| format!("Publish error: {:?}", e))
    }

    pub async fn run_event_loop(&mut self) {
        loop {
            select! {
                event = self.swarm.select_next_some() => {
                    match event {
                        SwarmEvent::NewListenAddr { address, .. } => {
                            info!("Listening on {:?}", address);
                        }
                        SwarmEvent::Behaviour(event) => {
                            // These are processed by NetworkBehaviourEventProcess implementations
                            // log::trace!("Swarm Behaviour event: {:?}", event); // Too verbose usually
                        }
                        SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                            info!("Connection established with: {:?}", peer_id);
                        }
                        SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                            warn!("Connection closed with: {:?}, cause: {:?}", peer_id, cause);
                        }
                        SwarmEvent::IncomingConnection { local_addr, send_back_addr } => {
                            info!("Incoming connection from {:?} to {:?}", send_back_addr, local_addr);
                        }
                        SwarmEvent::IncomingConnectionError { local_addr, send_back_addr, error } => {
                            error!("Incoming connection error from {:?} to {:?}: {:?}", send_back_addr, local_addr, error);
                        }
                        SwarmEvent::OutgoingConnectionError { peer_id, error } => {
                            error!("Outgoing connection error to {:?}: {:?}", peer_id, error);
                        }
                        SwarmEvent::Dialing(peer_id) => {
                             info!("Dialing peer: {:?}", peer_id);
                        }
                        // Other events can be handled here as needed
                        _ => {
                            // info!("Unhandled SwarmEvent: {:?}", event);
                        }
                    }
                }
                // External command to publish a message (example, not used directly here)
                // Some(external_cmd) = self.external_command_receiver.recv() => {
                //     // process external command
                // }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, timeout};
    use crate::data_structures::TransactionData; // Assuming TransactionData is needed for a message

    #[tokio::test]
    async fn test_network_manager_startup_and_shutdown() {
        let manager = NetworkManager::new(false).await; // Disable mDNS for this simple test
        assert!(manager.is_ok());
        let mut manager = manager.unwrap();

        // Run event loop for a short time to see if it panics
        let event_loop_handle = tokio::spawn(async move {
            manager.run_event_loop().await;
        });

        sleep(Duration::from_millis(100)).await; // Give it a moment to start
        event_loop_handle.abort(); // Stop the event loop
    }

    #[tokio::test]
    async fn test_message_publish_and_receive_two_nodes() {
        // Node 1
        let mut manager1 = NetworkManager::new(false).await.expect("Node 1 setup failed"); // mDNS off for predictability
        let peer_id1_str = manager1.swarm.local_peer_id().to_base58();
        let addr1 = match manager1.swarm.listeners().next() {
            Some(addr) => addr.clone(),
            None => panic!("Node 1 failed to start listening"),
        };
        info!("Node 1 ({}) listening on {}", peer_id1_str, addr1);

        // Node 2
        let mut manager2 = NetworkManager::new(false).await.expect("Node 2 setup failed");
        let peer_id2_str = manager2.swarm.local_peer_id().to_base58();
        let addr2 = match manager2.swarm.listeners().next() {
            Some(addr) => addr.clone(),
            None => panic!("Node 2 failed to start listening"),
        };
        info!("Node 2 ({}) listening on {}", peer_id2_str, addr2);

        // Connect Node 2 to Node 1
        manager2.swarm.dial(addr1.clone()).expect("Node 2 failed to dial Node 1");

        let node1_task = tokio::spawn(async move {
            manager1.run_event_loop().await;
            manager1 // Return manager to access receiver later if needed (though loop is infinite)
        });

        let node2_task = tokio::spawn(async move {
            manager2.run_event_loop().await;
            manager2 // Return manager
        });

        // Give some time for connection and gossipsub handshake
        sleep(Duration::from_secs(3)).await;


        // Node 1 sends a message
        // Retrieve manager1 from the task (this is a bit hacky for testing an infinite loop)
        // For a real test, you'd likely pass the sender part of the channel out.
        // For now, let's re-initialize a new manager and try to get the swarm from it to publish.
        // This part is problematic because the swarm is moved into the task.
        // A better approach for testing: The NetworkManager should provide a way to send messages
        // without needing to own the event loop, or the event loop should be pausable/stoppable.

        // Let's try to get the swarm from the task by aborting and recreating. This is not ideal.
        // A proper test would involve passing the sender channel of the behaviour to the test.

        // For now, let's assume we have a way to get a sender or directly publish on manager1's swarm.
        // Since we can't easily get manager1 back from the spawned task, let's try a different approach for the test.
        // We will create the message and then try to publish it using a new instance of the behaviour,
        // which is not how it works.

        // Corrected approach: The test needs to interact with the running NetworkManager instance.
        // We can't directly call `publish_message` on `manager1` as it's moved.
        // The `NetworkManager` should be designed to allow sending messages while its event loop is running.
        // This might involve an internal MPSC channel to send commands like "publish this message".

        // Simulating this for now by creating a new NetworkManager for publishing (which won't work as intended for gossip)
        // This test setup needs refinement. The core idea is to test that a message sent by one node
        // is received by another subscribed to the same topic.

        // Let's simplify: Node 1 will publish, Node 2 should receive.
        // We need to access manager1's publish_message and manager2's message_receiver.

        // This test requires a refactor of NetworkManager or a more complex setup.
        // Let's assume for a moment we can get `manager1` and `manager2` back or interact with them.
        // The current structure with `run_event_loop` taking `&mut self` and running infinitely
        // makes it hard to test externally like this.

        // Placeholder for actual message sending and receiving logic:
        let test_tx_data = TransactionData {
            to: Default::default(), from: Default::default(), user: "test_user".to_string(),
            sig: None, stake: 0.1, fee: 0.01,
        };
        let raw_tx_entry_content = RawTransactionEntry {
            tx_data: test_tx_data.clone(),
            validation_timestamps: vec![], validation_tasks: vec![], tx_timestamp: 0,
        };
        let message_to_send = ConsensusMessage::RawTransactionShare {
            from_node_id: manager.swarm.local_peer_id().to_base58(), // manager is Node 1
            raw_tx_id: "test_raw_tx_integration_1".to_string(),
            raw_tx_entry: raw_tx_entry_content.clone()
        };

        // Node 1 (manager) publishes the message
        manager.publish_message(&message_to_send).expect("Publish failed on manager1");

        // Check if Node 2 (manager2) received the message via its app_message_sender -> app_rx2
        match timeout(Duration::from_secs(10), app_rx2.recv()).await {
            Ok(Some(received_message)) => {
                info!("Node 2 received message: {:?}", received_message);
                match received_message {
                    ConsensusMessage::RawTransactionShare { raw_tx_entry: rec_entry, from_node_id, .. } => {
                        assert_eq!(rec_entry.tx_data, test_tx_data, "Transaction data mismatch");
                        assert_eq!(from_node_id, manager.swarm.local_peer_id().to_base58(), "Sender ID mismatch");
                    },
                    _ => panic!("Received unexpected message type on Node 2"),
                }
            }
            Ok(None) => panic!("Message channel (app_rx2) closed unexpectedly on Node 2"),
            Err(_) => panic!("Timeout waiting for message on Node 2 (app_rx2)"),
        }

        // Cleanup: Abort tasks
        manager1_task.abort();
        manager2_task.abort();
        // Wait for tasks to actually finish after aborting
        let _ = manager1_task.await;
        let _ = manager2_task.await;
    }
}
