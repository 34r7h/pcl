// Declare modules as public so they can be used by other crates
pub mod data_structures;
pub mod db;
pub mod network;
pub mod consensus_logic;

use log::{info, error};
use std::sync::Arc;
use tokio::sync::mpsc;

// Re-export key items for easier use by other crates
pub use data_structures::{TransactionData, NodeId, RawTxId, TxId}; // Add other important structs as needed
pub use consensus_logic::{ConsensusNode, ConsensusConfig};
pub use network::ConsensusMessage;


// This function will contain the logic previously in main.
pub async fn start_node() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init(); // Ensure logger is initialized when node starts
    info!("Starting Peer Consensus Node (from library function)...");

    let all_dbs = match db::AllMempoolDbs::new() {
        Ok(dbs) => {
            info!("All mempool databases initialized successfully.");
            Arc::new(dbs)
        }
        Err(e) => {
            error!("Failed to initialize mempool databases: {}", e);
            return Err(Box::new(e));
        }
    };

    let (to_network_sender, mut to_network_receiver) = mpsc::unbounded_channel::<network::ConsensusMessage>();
    let (to_consensus_sender, mut to_consensus_receiver) = mpsc::unbounded_channel::<network::ConsensusMessage>();

    let enable_mdns = true;
    let mut network_manager = match network::NetworkManager::new(enable_mdns, to_consensus_sender).await {
        Ok(nm) => {
            info!("NetworkManager initialized. Local Peer ID: {}", nm.swarm.local_peer_id());
            nm
        }
        Err(e) => {
            error!("Failed to initialize NetworkManager: {}", e);
            return Err(e.into());
        }
    };

    let node_id = network_manager.swarm.local_peer_id().to_base58();
    let consensus_config = consensus_logic::ConsensusConfig::default();
    // Note: ConsensusNode::new expects Arc<AllMempoolDbs>
    let consensus_node = Arc::new(ConsensusNode::new(node_id.clone(), Arc::clone(&all_dbs), to_network_sender, consensus_config));

    let consensus_handle = Arc::clone(&consensus_node);
    let consensus_task = tokio::spawn(async move {
        info!("Consensus logic task started for node: {}", consensus_handle.node_id);
        loop {
            match to_consensus_receiver.recv().await {
                Some(message) => {
                    // info!("Consensus logic received message: {:?}", message); // Can be too verbose
                    if let Err(e) = consensus_handle.process_network_message(message).await {
                        error!("Error processing network message in consensus logic: {}", e);
                    }
                }
                None => {
                    info!("Consensus logic channel closed for node: {}.", consensus_handle.node_id);
                    break;
                }
            }
        }
    });

    let network_task = tokio::spawn(async move {
        info!("Network manager task started for node: {}", node_id);
        loop {
            tokio::select! {
                event = network_manager.swarm.select_next_some() => {
                     match event {
                        libp2p::swarm::SwarmEvent::NewListenAddr { address, .. } => {
                            info!("Node {}: Listening on {:?}", node_id, address);
                        }
                        libp2p::swarm::SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                            info!("Node {}: Connection established with: {:?}", node_id, peer_id);
                        }
                        libp2p::swarm::SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                            warn!("Node {}: Connection closed with: {:?}, cause: {:?}", node_id, peer_id, cause);
                        }
                        _ => {}
                    }
                },
                Some(message_to_publish) = to_network_receiver.recv() => {
                    // info!("Node {}: Publishing message: {:?}", node_id, message_to_publish); // Can be too verbose
                    if let Err(e) = network_manager.publish_message(&message_to_publish) {
                        error!("Node {}: Failed to publish message: {}", node_id, e);
                    }
                },
                else => {
                    info!("NetworkManager channels closed for node: {}.", node_id);
                    break;
                }
            }
        }
    });

    // Optional: Example transaction simulation (can be removed or kept for testing the lib function)
    let example_consensus_node_for_sim = Arc::clone(&consensus_node);
     tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
        info!("Node {} (lib): Simulating a new transaction example", example_consensus_node_for_sim.node_id);
        let mut to_map = std::collections::HashMap::new();
        to_map.insert("bob_address_lib_sim".to_string(), 1.0);
        let mut from_map = std::collections::HashMap::new();
        from_map.insert("alice_utxo_lib_sim".to_string(), 1.3);
        let tx_data = data_structures::TransactionData {
            to: to_map, from: from_map, user: "alice_lib_sim".to_string(),
            sig: Some("alice_lib_sim_sig".to_string()), stake: 0.2, fee: 0.1,
        };
        match example_consensus_node_for_sim.handle_new_transaction_request(tx_data).await {
            Ok(raw_tx_id) => info!("Node {} (lib): Example transaction processed, raw_tx_id: {}", example_consensus_node_for_sim.node_id, raw_tx_id),
            Err(e) => error!("Node {} (lib): Example transaction failed: {}", example_consensus_node_for_sim.node_id, e),
        }
    });

    // The node will run indefinitely until the tasks are externally stopped or an error occurs.
    // If this `start_node` function is meant to be blocking, use `try_join!`.
    // If it's meant to start the node and return (non-blocking), then don't join here,
    // but the caller would need to manage the lifecycle.
    // For a library function that "starts a node", often it's non-blocking, returning handles if needed.
    // However, for simplicity here, let's make it blocking so a simple call to it runs the node.
    match tokio::try_join!(consensus_task, network_task) {
        Ok((_, _)) => info!("Node {} tasks finished successfully.", consensus_node.node_id),
        Err(e) => {
            error!("Node {} tasks failed: {}", consensus_node.node_id, e);
            return Err(Box::new(e));
        }
    }

    Ok(())
}
