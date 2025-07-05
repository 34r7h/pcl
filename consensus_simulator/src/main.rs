use clap::Parser;

/// Configuration for the Consensus Simulator
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct SimulatorConfig {
    /// Optional PeerId of a specific leader node to send transactions to.
    #[clap(long)]
    pub target_peer_id: Option<String>, // Assuming PeerId will be parsed from String

    /// Optional Multiaddr of a specific leader node to connect to.
    #[clap(long)]
    pub target_multiaddr: Option<String>, // Assuming Multiaddr will be parsed from String

    /// Number of unique user identities to simulate.
    #[clap(long, default_value_t = 10)]
    pub num_simulated_users: usize,

    /// Target rate for transaction submissions (transactions per second).
    #[clap(long, default_value_t = 1.0)]
    pub tx_rate_per_second: f64,

    /// Minimum transaction amount.
    #[clap(long, default_value_t = 1)]
    pub tx_amount_min: u64,

    /// Maximum transaction amount.
    #[clap(long, default_value_t = 1000)]
    pub tx_amount_max: u64,

    /// Minimum transaction fee.
    #[clap(long, default_value_t = 0.1)]
    pub tx_fee_min: f64,

    /// Maximum transaction fee.
    #[clap(long, default_value_t = 1.0)]
    pub tx_fee_max: f64,

    /// Minimum transaction stake.
    #[clap(long, default_value_t = 0.0)]
    pub tx_stake_min: f64,

    /// Maximum transaction stake.
    #[clap(long, default_value_t = 10.0)]
    pub tx_stake_max: f64,

    /// How long the simulation should run in seconds.
    #[clap(long, default_value_t = 60)]
    pub simulation_duration_secs: u64,

    /// Port for the simulator's libp2p node to listen on.
    #[clap(long, default_value_t = 0)] // 0 means OS assigns a port
    pub listen_port: u16,
}

mod user_manager;
mod transaction_generator;
mod p2p; // Added p2p module

use user_manager::UserManager;
use transaction_generator::TransactionGenerator;
use p2p::start_simulator_swarm; // Import the swarm starter
use consensus_node_lib::data_structures::P2PMessage; // For constructing the message to send
use libp2p::{futures::StreamExt, gossipsub::IdentTopic};
use tokio::time::{interval, Duration};
use std::sync::Arc; // For sharing config

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command-line arguments
    let config = Arc::new(SimulatorConfig::parse()); // Wrap config in Arc for sharing

    println!("[Simulator] Configuration: {:?}", config);

    // Initialize User Manager
    let user_manager = Arc::new(UserManager::new(config.num_simulated_users));
    println!("[Simulator] Initialized {} simulated users.", user_manager.get_user_count());

    // Initialize Transaction Generator
    let transaction_generator = Arc::new(TransactionGenerator::new());
    println!("[Simulator] Initialized Transaction Generator.");

    // Initialize and start libp2p swarm for the simulator
    let mut swarm = start_simulator_swarm(&config).await?;
    println!("[Simulator] Libp2p swarm started.");

    println!("[Simulator] Starting with {} users, {:.2} TPS, for {} seconds.",
             config.num_simulated_users, config.tx_rate_per_second, config.simulation_duration_secs);

    let topic = IdentTopic::new("consensus-messages");

    let mut tx_counter: u64 = 0; // To ensure unique UTXO IDs per sender over time

    // Transaction Sending Loop
    let mut send_interval = interval(Duration::from_secs_f64(1.0 / config.tx_rate_per_second.max(0.01))); // Ensure rate > 0
    let simulation_end_time = tokio::time::Instant::now() + Duration::from_secs(config.simulation_duration_secs);

    loop {
        tokio::select! {
            _ = send_interval.tick() => {
                if tokio::time::Instant::now() >= simulation_end_time {
                    println!("[Simulator] Simulation duration reached. Stopping transaction generation.");
                    break;
                }

                let sender = user_manager.get_next_user();
                // For recipient, let's pick another user. If only one user, they send to themselves.
                let recipient = if user_manager.get_user_count() > 1 {
                    loop {
                        let r = user_manager.get_next_user(); // Use round-robin to vary recipients
                        if r.public_key_hex != sender.public_key_hex { // Avoid self-sending if possible
                            break r;
                        }
                        // If only one other user and it's the sender, this loop might be tight.
                        // For >2 users, this works. For 2 users, it alternates. For 1, self-send.
                        if user_manager.get_user_count() <= 2 && r.public_key_hex == sender.public_key_hex {
                             break r; // allow self send if only one user or stuck
                        }
                    }
                } else {
                    sender // Self-transaction if only one user
                };

                tx_counter += 1;
                let tx_data = transaction_generator.generate_transaction(
                    sender,
                    recipient.public_key_hex.clone(),
                    &config,
                    tx_counter
                );

                let p2p_message = P2PMessage::ClientSubmitRawTransaction(tx_data.clone());
                match serde_json::to_vec(&p2p_message) {
                    Ok(serialized_message) => {
                        if let Err(e) = swarm.behaviour_mut().gossipsub.publish(topic.clone(), serialized_message) {
                            eprintln!("[Simulator] Failed to publish transaction gossip: {:?}", e);
                        } else {
                            println!("[Simulator] Gossiped transaction {} from user {} to user {}.",
                                     tx_data.calculate_hash(),
                                     sender.public_key_hex,
                                     recipient.public_key_hex);
                        }
                    }
                    Err(e) => {
                        eprintln!("[Simulator] Failed to serialize P2PMessage for transaction: {:?}", e);
                    }
                }
            },
            event = swarm.select_next_some() => {
                // Handle libp2p swarm events (like mDNS discoveries, etc.)
                // The SimulatorBehaviour already logs mDNS events.
                // Add more handling here if needed.
                match event {
                    libp2p::swarm::SwarmEvent::NewListenAddr { address, .. } => {
                        println!("[Simulator] Listening on {}", address);
                    }
                    libp2p::swarm::SwarmEvent::Behaviour(event) => {
                        // Specific behaviour events can be handled here if SimulatorBehaviour emits them
                        // println!("[Simulator] Behaviour event: {:?}", event);
                    }
                    libp2p::swarm::SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                        println!("[Simulator] Connection established with: {}", peer_id);
                    }
                    libp2p::swarm::SwarmEvent::ConnectionClosed { peer_id, cause,.. } => {
                        println!("[Simulator] Connection to {} closed, cause: {:?}", peer_id, cause.map(|c| c.to_string()));
                    }
                    _ => {} // Ignore other events for now
                }
            },
            _ = tokio::signal::ctrl_c() => {
                println!("[Simulator] Ctrl-C received, shutting down.");
                break;
            }
        }
    }

    println!("[Simulator] Finished.");
    Ok(())
}
```
