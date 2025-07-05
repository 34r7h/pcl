use peer_consensus_node::{start_node, TransactionData, NodeId}; // Assuming NodeId might be useful
use log::{info, error, warn};
use std::collections::HashMap;
use std::time::Duration;
use rand::Rng;
use tokio::task::JoinHandle;

// Simulator Configuration
#[derive(Debug, Clone)]
struct SimulatorConfig {
    num_nodes: usize, // For now, we'll only effectively use 1
    num_clients: usize,
    transactions_per_client: usize,
    tx_submission_interval_ms: u64,
    // Potentially, addresses or connection details for nodes if run as separate processes
    // For in-process, we might pass around node handles or communication channels.
}

impl Default for SimulatorConfig {
    fn default() -> Self {
        SimulatorConfig {
            num_nodes: 1, // Start with one node for simplicity
            num_clients: 5,
            transactions_per_client: 10,
            tx_submission_interval_ms: 1000, // 1 tx per second per client
        }
    }
}

async fn run_simulated_client(client_id: usize, config: SimulatorConfig, target_node_id: Option<NodeId>) {
    info!("[Client {}] Starting...", client_id);
    let mut rng = rand::thread_rng();

    // How to send a transaction?
    // Option 1: HTTP API (node needs to expose one)
    // Option 2: Direct call if node is in-process and we have a handle to its ConsensusNode instance or a channel.
    // Option 3: Via libp2p message (would require client to be a libp2p node itself, complex for simple simulator client)

    // For now, let's assume we need to construct TransactionData and somehow "send" it.
    // The `handle_new_transaction_request` is on `ConsensusNode`.
    // If `start_node` gives us a way to get a handle to the `ConsensusNode` or a sender channel to it, we could use that.
    // The current `start_node` is blocking and doesn't return such a handle easily.

    // Placeholder: We'll just log the transactions we *would* send.
    for i in 0..config.transactions_per_client {
        let mut to_map = HashMap::new();
        let recipient_addr = format!("sim_recipient_{}", rng.gen_range(0..100));
        to_map.insert(recipient_addr, rng.gen_range(0.1..10.0));

        let mut from_map = HashMap::new();
        let utxo_id = format!("sim_utxo_client{}_tx{}", client_id, i);
        from_map.insert(utxo_id.clone(), rng.gen_range(1.0..20.0)); // Ensure enough funds

        let tx_data = TransactionData {
            to: to_map,
            from: from_map,
            user: format!("sim_user_client{}", client_id),
            sig: Some(format!("sim_sig_client{}_tx{}", client_id, i)),
            stake: rng.gen_range(0.01..0.5),
            fee: rng.gen_range(0.001..0.1),
        };

        if let Some(node_id) = &target_node_id {
            info!("[Client {}] Would send tx {} to node {}: User: {}, UTXO: {}, Amount: {:.2}",
                client_id, i, node_id, tx_data.user, utxo_id, tx_data.to.values().sum::<f64>()
            );
            // TODO: Actually send the transaction to the target_node_id
            // This requires an API on the node. For example, if the node had an HTTP endpoint:
            // let client = reqwest::Client::new();
            // match client.post(&format!("http://{}/submit_transaction", node_api_address)) // node_api_address would be needed
            //     .json(&tx_data)
            //     .send()
            //     .await {
            //         Ok(resp) => info!("[Client {}] Tx {} submission response: {:?}", client_id, i, resp.status()),
            //         Err(e) => error!("[Client {}] Tx {} submission error: {}", client_id, i, e),
            // }
        } else {
            warn!("[Client {}] No target node ID available to send tx {}.", client_id, i);
        }


        tokio::time::sleep(Duration::from_millis(config.tx_submission_interval_ms)).await;
    }
    info!("[Client {}] Finished sending transactions.", client_id);
}


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    info!("Starting Consensus Simulator...");

    let config = SimulatorConfig::default();

    // --- Node Spawning ---
    // The current `peer_consensus_node::start_node()` is blocking and uses fixed paths/ports.
    // To run multiple nodes, it needs to be refactored to accept config for DB paths, listen addresses, etc.
    // And to be non-blocking or return handles.

    // For now, let's try to spawn ONE node in a separate tokio task.
    info!("Simulator: Attempting to start one peer_consensus_node instance...");
    let node_handle: JoinHandle<Result<(), Box<dyn std::error::Error + Send>>> = tokio::spawn(async {
        // `start_node` needs to be `Send` if spawned this way.
        // The error type from start_node also needs to be Send.
        // Box<dyn Error> is Send if the underlying error is Send.
        if let Err(e) = peer_consensus_node::start_node().await {
            error!("Simulator: Node task failed: {}", e);
            return Err(e); // Propagate the error
        }
        Ok(())
    });

    // Give the node some time to start up.
    // In a real scenario, we'd need a more robust way to check if the node is ready (e.g., health check endpoint).
    info!("Simulator: Waiting for the node to initialize (e.g., 15 seconds)...");
    tokio::time::sleep(Duration::from_secs(15)).await;

    // We need the NodeId of the spawned node to target transactions.
    // The `start_node` function currently prints its PeerId, but doesn't return it directly.
    // This is a limitation for the simulator. For now, we can't easily get the NodeId.
    // Let's assume we will get it somehow, or clients will send to a default/known address.
    // The example transaction inside `start_node` uses its own generated node_id.
    // We can't easily get that.
    // So, for now, `target_node_id` will be None for clients.
    let target_node_id_for_clients: Option<NodeId> = None;
    warn!("Simulator: Target Node ID for clients is not yet dynamically obtained. Transaction sending will be logged only.");


    // --- Client Simulation ---
    info!("Simulator: Spawning {} clients...", config.num_clients);
    let mut client_handles = Vec::new();
    for i in 0..config.num_clients {
        let client_config = config.clone();
        // Each client needs to know how to contact the node(s).
        // This is where the node's API address or libp2p PeerId would be used.
        let handle = tokio::spawn(run_simulated_client(i, client_config, target_node_id_for_clients.clone()));
        client_handles.push(handle);
    }

    // Wait for all clients to finish
    for handle in client_handles {
        handle.await?;
    }
    info!("Simulator: All clients finished.");

    // --- Shutdown ---
    // How to gracefully shut down the spawned node?
    // If `node_handle` is for a blocking `start_node`, we might need to abort it.
    // Or `start_node` needs a shutdown signal mechanism.
    info!("Simulator: Simulating work complete. Aborting node task (if still running)...");
    node_handle.abort(); // This will cause the task to be cancelled.
    match node_handle.await {
        Ok(Ok(_)) => info!("Simulator: Node task completed successfully after abort (or finished early)."),
        Ok(Err(e)) => info!("Simulator: Node task panicked or returned an error after abort: {}", e),
        Err(e) => info!("Simulator: Node task join error (likely due to abort): {}", e), // This is expected on abort.
    }

    info!("Consensus Simulator finished.");
    Ok(())
}
