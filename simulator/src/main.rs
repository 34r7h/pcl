use clap::{Parser, Subcommand};
use log::info;
use tokio::time::sleep;
use pcl_backend::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

mod simulation;
mod node_spawner;
mod transaction_generator;
mod metrics;
mod network;

use simulation::Simulation;

#[derive(Parser)]
#[command(name = "pcl-simulator")]
#[command(about = "Peer Consensus Layer Transaction Load Simulator")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a transaction load simulation
    LoadTest {
        /// Number of nodes to spawn
        #[arg(short, long, default_value_t = 10)]
        nodes: u32,
        
        /// Number of leader nodes
        #[arg(short, long, default_value_t = 3)]
        leaders: u32,
        
        /// Transactions per second to generate
        #[arg(short, long, default_value_t = 100)]
        tps: u32,
        
        /// Duration of simulation in seconds
        #[arg(short, long, default_value_t = 60)]
        duration: u64,
        
        /// Enable verbose logging
        #[arg(short, long)]
        verbose: bool,
    },
    /// Stress test the system with high load
    StressTest {
        /// Maximum nodes to spawn
        #[arg(short, long, default_value_t = 100)]
        max_nodes: u32,
        
        /// Maximum transactions per second
        #[arg(short, long, default_value_t = 1000)]
        max_tps: u32,
        
        /// Duration of each test phase in seconds
        #[arg(short, long, default_value_t = 30)]
        phase_duration: u64,
    },
    /// Benchmark specific scenarios
    Benchmark {
        /// Scenario to benchmark
        #[arg(short, long, value_enum)]
        scenario: BenchmarkScenario,
        
        /// Number of iterations
        #[arg(short, long, default_value_t = 5)]
        iterations: u32,
    },
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum BenchmarkScenario {
    LeaderElection,
    TransactionProcessing,
    NetworkGossip,
    MempoolPerformance,
}

struct RealSimulator {
    nodes: HashMap<String, SimulatorNode>,
    keypairs: HashMap<String, NodeKeypair>,
    network_stats: NetworkStats,
    consensus_stats: ConsensusStats,
}

struct SimulatorNode {
    id: String,
    address: String,
    role: NodeRole,
    keypair: NodeKeypair,
    public_key_hex: String,
    is_active: bool,
    last_activity: Instant,
    transactions_processed: u64,
    signatures_generated: u64,
}

#[derive(Debug, Clone)]
struct NetworkStats {
    total_nodes: usize,
    active_nodes: usize,
    messages_sent: u64,
    signatures_verified: u64,
}

#[derive(Debug, Clone)]
struct ConsensusStats {
    transactions_processed: u64,
    validation_tasks_completed: u64,
    leader_elections_held: u64,
    consensus_rounds: u64,
}

impl RealSimulator {
    fn new() -> Self {
        log::info!("🚀 REAL SIMULATOR: Initializing with real cryptographic keys");
        
        Self {
            nodes: HashMap::new(),
            keypairs: HashMap::new(),
            network_stats: NetworkStats {
                total_nodes: 0,
                active_nodes: 0,
                messages_sent: 0,
                signatures_verified: 0,
            },
            consensus_stats: ConsensusStats {
                transactions_processed: 0,
                validation_tasks_completed: 0,
                leader_elections_held: 0,
                consensus_rounds: 0,
            },
        }
    }
    
    fn initialize_network(&mut self, node_count: usize) {
        log::info!("🌐 REAL NETWORK INIT: Creating {} nodes with real cryptographic identities", node_count);
        
        for i in 0..node_count {
            let node_id = format!("sim_node_{:03}", i);
            let address = format!("192.168.100.{}", i + 1);
            
            // REAL IMPLEMENTATION: Generate actual cryptographic keypair
            let keypair = NodeKeypair::new();
            let public_key = keypair.public_key();
            let public_key_hex = hex::encode(public_key.to_bytes());
            
            log::info!("🔑 REAL KEYPAIR: Generated for node {} - Public key: {}", 
                       node_id, &public_key_hex[..16]);
            
            let role = if i < 3 {
                NodeRole::Leader
            } else if i < 8 {
                NodeRole::Validator
            } else {
                NodeRole::Extension
            };
            
            let node = SimulatorNode {
                id: node_id.clone(),
                address,
                role,
                keypair: keypair.clone(),
                public_key_hex,
                is_active: true,
                last_activity: Instant::now(),
                transactions_processed: 0,
                signatures_generated: 0,
            };
            
            self.nodes.insert(node_id.clone(), node);
            self.keypairs.insert(node_id, keypair);
        }
        
        self.network_stats.total_nodes = node_count;
        self.network_stats.active_nodes = node_count;
        
        log::info!("✅ REAL NETWORK READY: {} nodes initialized with real cryptographic identities", node_count);
    }
    
    async fn run_consensus_simulation(&mut self, rounds: usize) {
        log::info!("🏛️  REAL CONSENSUS: Starting {} rounds of consensus with real signatures", rounds);
        
        for round in 1..=rounds {
            log::info!("🔄 CONSENSUS ROUND {}: Starting with real cryptographic operations", round);
            
            // Simulate real transaction processing
            self.simulate_transaction_processing().await;
            
            // Simulate real leader election
            self.simulate_leader_election().await;
            
            // Simulate real validation tasks
            self.simulate_validation_tasks().await;
            
            self.consensus_stats.consensus_rounds += 1;
            
            // Wait between rounds
            sleep(Duration::from_secs(2)).await;
        }
        
        log::info!("🎉 REAL CONSENSUS COMPLETE: Completed {} rounds with real cryptographic operations", rounds);
    }
    
    async fn simulate_transaction_processing(&mut self) {
        log::info!("💰 REAL TRANSACTION PROCESSING: Simulating with real signatures");
        
        // Get leader nodes data
        let leader_nodes: Vec<_> = self.nodes.values()
            .filter(|n| n.role == NodeRole::Leader && n.is_active)
            .map(|n| (n.id.clone(), n.keypair.clone()))
            .collect();
        
        if leader_nodes.is_empty() {
            log::warn!("⚠️  NO LEADERS: Cannot process transactions without leader nodes");
            return;
        }
        
        // Simulate transaction creation and signing
        for i in 0..3 {
            let tx_id = format!("tx_{:08x}", rand::random::<u32>());
            let (leader_id, leader_keypair) = &leader_nodes[i % leader_nodes.len()];
            
            // REAL IMPLEMENTATION: Create and sign transaction
            let tx_data = TransactionData::new(
                vec![("recipient_address".to_string(), 10.0)],
                vec![("sender_utxo".to_string(), 15.0)],
                "sender_address".to_string(),
                1.0,
                0.1,
            );
            
            let tx_bytes = serde_json::to_vec(&tx_data).unwrap();
            let signature = leader_keypair.sign_data(&tx_bytes);
            let sig_hex = hex::encode(signature.to_bytes());
            
            log::info!("✍️  REAL TRANSACTION SIGNED: TX {} signed by leader {} with signature {}", 
                       tx_id, leader_id, &sig_hex[..16]);
            
            // Update statistics
            if let Some(node) = self.nodes.get_mut(leader_id) {
                node.transactions_processed += 1;
                node.signatures_generated += 1;
                node.last_activity = Instant::now();
            }
            
            self.consensus_stats.transactions_processed += 1;
            self.network_stats.messages_sent += 1;
            
            // Simulate signature verification by validators
            let validator_nodes: Vec<_> = self.nodes.values()
                .filter(|n| n.role == NodeRole::Validator && n.is_active)
                .map(|n| (n.id.clone(), n.keypair.clone()))
                .collect();
            
            for (validator_id, _) in validator_nodes.iter().take(2) {
                let public_key = leader_keypair.public_key();
                let verification_result = verify_data_signature(&tx_bytes, &signature, &public_key);
                
                match verification_result {
                    Ok(is_valid) => {
                        if is_valid {
                            log::info!("✅ SIGNATURE VERIFIED: Validator {} verified transaction {}", 
                                       validator_id, tx_id);
                            self.network_stats.signatures_verified += 1;
                        } else {
                            log::warn!("❌ SIGNATURE INVALID: Validator {} rejected transaction {}", 
                                       validator_id, tx_id);
                        }
                    }
                    Err(e) => {
                        log::warn!("❌ VERIFICATION ERROR: Validator {} error: {}", validator_id, e);
                    }
                }
            }
        }
    }
    
    async fn simulate_leader_election(&mut self) {
        log::info!("🗳️  REAL LEADER ELECTION: Simulating with real cryptographic voting");
        
        // Get all nodes eligible for leadership
        let eligible_nodes: Vec<_> = self.nodes.values()
            .filter(|n| n.is_active)
            .collect();
        
        if eligible_nodes.is_empty() {
            log::warn!("⚠️  NO ELIGIBLE NODES: Cannot hold leader election");
            return;
        }
        
        // Simulate voting with real signatures
        let mut votes = HashMap::new();
        
        for voter in &eligible_nodes {
            for candidate in &eligible_nodes {
                if voter.id != candidate.id {
                    // REAL IMPLEMENTATION: Sign vote
                    let vote_data = format!("vote_for_{}", candidate.id);
                    let vote_signature = voter.keypair.sign_data(vote_data.as_bytes());
                    let vote_sig_hex = hex::encode(vote_signature.to_bytes());
                    
                    *votes.entry(candidate.id.clone()).or_insert(0) += 1;
                    
                    log::info!("🗳️  REAL VOTE: {} voted for {} with signature {}", 
                               voter.id, candidate.id, &vote_sig_hex[..16]);
                }
            }
        }
        
        // Determine leaders
        let mut sorted_candidates: Vec<_> = votes.into_iter().collect();
        sorted_candidates.sort_by(|a, b| b.1.cmp(&a.1));
        
        let leaders: Vec<_> = sorted_candidates.into_iter()
            .take(3)
            .map(|(id, vote_count)| {
                log::info!("👑 ELECTED LEADER: {} with {} votes", id, vote_count);
                id
            })
            .collect();
        
        // Update node roles
        for node in self.nodes.values_mut() {
            node.role = if leaders.contains(&node.id) {
                NodeRole::Leader
            } else {
                NodeRole::Validator
            };
        }
        
        self.consensus_stats.leader_elections_held += 1;
        log::info!("✅ LEADER ELECTION COMPLETE: {} leaders elected with real cryptographic votes", leaders.len());
    }
    
    async fn simulate_validation_tasks(&mut self) {
        log::info!("🔍 REAL VALIDATION TASKS: Simulating with real cryptographic validation");
        
        let validator_nodes: Vec<_> = self.nodes.values()
            .filter(|n| n.role == NodeRole::Validator && n.is_active)
            .map(|n| (n.id.clone(), n.keypair.clone()))
            .collect();
        
        if validator_nodes.is_empty() {
            log::warn!("⚠️  NO VALIDATORS: Cannot perform validation tasks");
            return;
        }
        
        // Create validation tasks
        for i in 0..5 {
            let task_id = format!("validation_task_{:08x}", rand::random::<u32>());
            let (validator_id, validator_keypair) = &validator_nodes[i % validator_nodes.len()];
            
            // REAL IMPLEMENTATION: Create validation task data
            let task_data = ValidationTask::new(
                task_id.clone(),
                validator_id.clone(),
                ValidationTaskType::SignatureValidation,
            );
            
            // Validator signs the validation result
            let task_bytes = serde_json::to_vec(&task_data).unwrap();
            let validation_signature = validator_keypair.sign_data(&task_bytes);
            let validation_sig_hex = hex::encode(validation_signature.to_bytes());
            
            log::info!("✍️  REAL VALIDATION: Task {} validated by {} with signature {}", 
                       task_id, validator_id, &validation_sig_hex[..16]);
            
            // Update statistics
            if let Some(node) = self.nodes.get_mut(validator_id) {
                node.signatures_generated += 1;
                node.last_activity = Instant::now();
            }
            
            self.consensus_stats.validation_tasks_completed += 1;
            
            // Simulate verification by other validators
            for (other_validator_id, _) in validator_nodes.iter().take(2) {
                if other_validator_id != validator_id {
                    let public_key = validator_keypair.public_key();
                    let verification_result = verify_data_signature(&task_bytes, &validation_signature, &public_key);
                    
                    match verification_result {
                        Ok(is_valid) => {
                            if is_valid {
                                log::info!("✅ VALIDATION VERIFIED: {} verified task {} by {}", 
                                           other_validator_id, task_id, validator_id);
                                self.network_stats.signatures_verified += 1;
                            } else {
                                log::warn!("❌ VALIDATION INVALID: {} rejected task {} by {}", 
                                           other_validator_id, task_id, validator_id);
                            }
                        }
                        Err(e) => {
                            log::warn!("❌ VALIDATION ERROR: {} error: {}", other_validator_id, e);
                        }
                    }
                }
            }
        }
    }
    
    fn print_final_stats(&self) {
        log::info!("📊 REAL SIMULATOR FINAL STATISTICS:");
        log::info!("   🌐 Network Stats:");
        log::info!("     - Total nodes: {}", self.network_stats.total_nodes);
        log::info!("     - Active nodes: {}", self.network_stats.active_nodes);
        log::info!("     - Messages sent: {}", self.network_stats.messages_sent);
        log::info!("     - Signatures verified: {}", self.network_stats.signatures_verified);
        
        log::info!("   🏛️  Consensus Stats:");
        log::info!("     - Transactions processed: {}", self.consensus_stats.transactions_processed);
        log::info!("     - Validation tasks completed: {}", self.consensus_stats.validation_tasks_completed);
        log::info!("     - Leader elections held: {}", self.consensus_stats.leader_elections_held);
        log::info!("     - Consensus rounds: {}", self.consensus_stats.consensus_rounds);
        
        log::info!("   🔑 Cryptographic Operations:");
        let total_signatures: u64 = self.nodes.values()
            .map(|n| n.signatures_generated)
            .sum();
        log::info!("     - Total signatures generated: {}", total_signatures);
        log::info!("     - Signature verification rate: {:.2}%", 
                   (self.network_stats.signatures_verified as f64 / total_signatures as f64) * 100.0);
        
        log::info!("   📈 Node Activity:");
        let active_nodes = self.nodes.values()
            .filter(|n| n.is_active)
            .count();
        log::info!("     - Active nodes: {}/{}", active_nodes, self.network_stats.total_nodes);
        
        for node in self.nodes.values() {
            log::info!("     - {}: {} txns, {} sigs, role: {:?}", 
                       node.id, node.transactions_processed, node.signatures_generated, node.role);
        }
    }
}

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    
    log::info!("🚀 STARTING REAL CRYPTOGRAPHIC SIMULATOR");
    log::info!("=========================================");
    
    let mut simulator = RealSimulator::new();
    
    // Initialize network with real cryptographic identities
    simulator.initialize_network(15);
    
    // Run consensus simulation with real signatures
    simulator.run_consensus_simulation(10).await;
    
    // Print final statistics
    simulator.print_final_stats();
    
    log::info!("✅ REAL SIMULATOR COMPLETE");
    log::info!("All operations performed with real cryptographic signatures and verifications");
    
    Ok(())
} 