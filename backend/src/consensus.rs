// Consensus module - TODO: Implement consensus functionality 

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Mutex};
use tokio::time::{sleep, interval};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use serde::{Deserialize, Serialize};

use crate::error::{PclError, Result};
use crate::node::{Node, NodeRole, NodeRegistry};
use crate::transaction::{RawTransaction, ValidationTask, ValidationTaskType, ProcessingTransaction, TransactionData};
use crate::mempool::{MempoolManager, FinalizedTransaction};
use crate::network::{NetworkManager, NetworkMessage, TransactionGossipMessage, ValidationTaskMessage, LeaderElectionMessage, PulseMessage, PulseResponseMessage, UptimeMessage};
use crate::storage::StorageManager;
use crate::crypto::{NodeKeypair, sign_data, hash_data};

// Main consensus manager
pub struct ConsensusManager {
    pub node_registry: Arc<RwLock<NodeRegistry>>,
    pub mempool: Arc<RwLock<MempoolManager>>,
    pub network_manager: Arc<Mutex<NetworkManager>>,
    pub storage_manager: Arc<StorageManager>,
    pub local_node: Node, // Represents the conceptual node identity
    pub local_peer_id: String, // libp2p PeerId of this node
    pub local_node_keypair: Arc<NodeKeypair>, // Added for signing
    pub leader_election: Arc<RwLock<LeaderElectionManager>>,
    pub pulse_system: Arc<RwLock<PulseSystem>>,
    pub transaction_processor: Arc<RwLock<TransactionProcessor>>,
    pub validation_engine: Arc<RwLock<ValidationEngine>>,
    pub consensus_state: Arc<RwLock<ConsensusState>>,
}

// Leader election manager
#[derive(Debug, Clone)]
pub struct LeaderElectionManager {
    pub current_leaders: Vec<String>,
    pub election_round: u64,
    pub last_election_time: DateTime<Utc>,
    pub voting_data: HashMap<String, VotingData>,
    pub broadcasting_cycle: Arc<RwLock<BroadcastingCycle>>,
}

// Helper struct for leader election candidates - internal to ConsensusManager logic
#[derive(Debug, Clone)]
struct CandidateInfo {
    node_uuid: String, // Application Node UUID
    performance_score: f64,
    uptime_score: f64,
    combined_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VotingData {
    pub candidate_id: String, // Application Node UUID
    pub votes: u64,
    pub performance_score: f64,
    pub uptime_score: f64,
    pub round: u8,
}

#[derive(Debug, Clone)]
pub struct BroadcastingCycle {
    pub cycle_start: DateTime<Utc>,
    pub cycle_duration_hours: u64,
    pub current_leaders: Vec<String>,
}

// Pulse system for uptime tracking
#[derive(Debug, Clone)]
pub struct PulseSystem {
    pub pulse_interval_seconds: u64,
    pub family_assignments: HashMap<String, Uuid>, // node_id -> family_id
    pub pulse_data: HashMap<String, PulseData>,
    pub response_times: HashMap<String, Vec<u64>>, // node_id -> response_times_ms
    pub last_pulse_time: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PulseData {
    pub node_id: String,
    pub family_id: Uuid,
    pub pulse_count: u64,
    pub average_response_time_ms: f64,
    pub uptime_percentage: f64,
    pub last_pulse: DateTime<Utc>,
}

// Transaction processing engine
#[derive(Debug, Clone)]
pub struct TransactionProcessor {
    pub processing_queue: Vec<RawTransaction>,
    pub validation_assignments: HashMap<String, Vec<ValidationTask>>, // tx_id -> tasks
    pub average_timestamps: HashMap<String, DateTime<Utc>>,
    pub leader_signatures: HashMap<String, String>,
}

// Validation engine
#[derive(Debug, Clone)]
pub struct ValidationEngine {
    pub active_tasks: HashMap<String, ValidationTask>,
    pub completed_tasks: HashMap<String, ValidationTask>,
    pub validation_results: HashMap<String, ValidationResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub task_id: String,
    pub tx_id: String,
    pub validation_type: ValidationTaskType,
    pub success: bool,
    pub error_message: Option<String>,
    pub completed_at: DateTime<Utc>,
}

// Overall consensus state
#[derive(Debug, Clone)]
pub struct ConsensusState {
    pub current_phase: ConsensusPhase,
    pub active_transactions: HashMap<String, TransactionWorkflowState>,
    pub leader_performance: HashMap<String, LeaderPerformance>,
    pub system_load: f64,
    pub network_health: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConsensusPhase {
    Initialization,
    NormalOperation,
    LeaderElection,
    NetworkPartition,
    Recovery,
}

#[derive(Debug, Clone)]
pub struct TransactionWorkflowState {
    pub tx_id: String,
    pub current_step: u8,
    pub workflow_data: TransactionWorkflowData,
    pub start_time: DateTime<Utc>,
    pub last_update: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionWorkflowData {
    pub alice_transaction: Option<RawTransaction>,
    pub charlie_processing: Option<ProcessingTransaction>,
    pub validation_tasks: Vec<ValidationTask>,
    pub alice_completion: Option<DateTime<Utc>>,
    pub charlie_final_processing: Option<DateTime<Utc>>,
    pub validator_broadcast: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct LeaderPerformance {
    pub node_id: String,
    pub transactions_processed: u64,
    pub validation_tasks_assigned: u64,
    pub average_processing_time_ms: f64,
    pub uptime_percentage: f64,
    pub performance_score: f64,
}

impl ConsensusManager {
    pub fn new(
        local_node: Node,
        local_peer_id: String, // Added for libp2p
        local_node_keypair: NodeKeypair, // Added for signing
        network_manager: NetworkManager,
        storage_manager: StorageManager,
    ) -> Result<Self> {
        let node_registry = Arc::new(RwLock::new(NodeRegistry::new()));
        let mempool = Arc::new(RwLock::new(MempoolManager::new()));
        let network_manager = Arc::new(Mutex::new(network_manager));
        let storage_manager = Arc::new(storage_manager);
        let local_node_keypair = Arc::new(local_node_keypair);
        
        let leader_election = Arc::new(RwLock::new(LeaderElectionManager::new()));
        let pulse_system = Arc::new(RwLock::new(PulseSystem::new()));
        let transaction_processor = Arc::new(RwLock::new(TransactionProcessor::new()));
        let validation_engine = Arc::new(RwLock::new(ValidationEngine::new()));
        let consensus_state = Arc::new(RwLock::new(ConsensusState::new()));

        Ok(ConsensusManager {
            node_registry,
            mempool,
            network_manager,
            storage_manager,
            local_node,
            local_peer_id,
            local_node_keypair,
            leader_election,
            pulse_system,
            transaction_processor,
            validation_engine,
            consensus_state,
        })
    }

    pub async fn start(&self) -> Result<()> {
        log::info!("Starting consensus manager for node: {}", self.local_node.id);
        
        // Initialize consensus state
        let mut state = self.consensus_state.write().await;
        state.current_phase = ConsensusPhase::Initialization;
        drop(state);
        
        // Start background tasks
        self.start_pulse_system().await?;
        self.start_leader_election_cycle().await?;
        self.start_transaction_processing().await?;
        self.start_validation_engine().await?;
        
        // Set to normal operation
        let mut state = self.consensus_state.write().await;
        state.current_phase = ConsensusPhase::NormalOperation;
        drop(state);
        
        log::info!("Consensus manager started successfully");
        Ok(())
    }

    // Transaction workflow implementation (6 steps from README)
    pub async fn process_transaction_workflow(&self, tx: RawTransaction) -> Result<()> {
        log::info!("Starting transaction workflow for tx: {}", tx.raw_tx_id);
        
        // Step 1: Alice creates transaction
        let workflow_state = self.step1_alice_creates_transaction(tx).await?;
        
        // Step 2: Charlie processes and gossips
        let workflow_state = self.step2_charlie_processes_transaction(workflow_state).await?;
        
        // Step 3: Leaders assign validation tasks
        let workflow_state = self.step3_leaders_assign_validation_tasks(workflow_state).await?;
        
        // Step 4: Alice completes validation tasks
        let workflow_state = self.step4_alice_completes_validation_tasks(workflow_state).await?;
        
        // Step 5: Charlie processes validation results
        let workflow_state = self.step5_charlie_processes_validation(workflow_state).await?;
        
        // Step 6: Validator broadcasts and finalizes
        self.step6_validator_broadcasts_and_finalizes(workflow_state).await?;
        
        log::info!("Transaction workflow completed successfully");
        Ok(())
    }

    async fn step1_alice_creates_transaction(&self, mut tx: RawTransaction) -> Result<TransactionWorkflowState> {
        log::debug!("Step 1: Alice creates transaction {}", tx.raw_tx_id);

        // Validate Alice's signature
        // tx.tx_data.user is expected to be the hex string of Alice's public key
        let alice_public_key_hex = tx.tx_data.user.clone();
        
        let node_registry = self.node_registry.read().await;
        // Find Alice's node by iterating through the registry and matching public key hex.
        // This is inefficient; a map in NodeRegistry from pubkey_hex to Node would be better.
        let alice_node_opt = node_registry.nodes.values().find(|n| {
            hex::encode(n.public_key.as_bytes()) == alice_public_key_hex
        });

        let alice_node = alice_node_opt
            .ok_or_else(|| PclError::NodeNotFound(format!("Alice's node with pubkey hex {} not found in registry", alice_public_key_hex)))?;

        if !tx.tx_data.validate_signature(&alice_node.public_key) {
            log::warn!("Invalid signature for transaction {} from user (pubkey hex {})", tx.raw_tx_id, alice_public_key_hex);
            return Err(PclError::InvalidSignature("Alice's transaction signature is invalid".to_string()));
        }
        log::info!("Alice's signature validated for transaction {}", tx.raw_tx_id);

        // Ensure the transaction's user field (Alice's pubkey hex) is correctly set.
        // tx.tx_data.user should already be this from the sender.

        // Add to raw transaction mempool
        let mut mempool = self.mempool.write().await;
        mempool.add_raw_transaction(tx.clone())?;
        drop(mempool);
        
        // Store in database
        self.storage_manager.store_raw_transaction(&tx)?;
        
        let workflow_state = TransactionWorkflowState {
            tx_id: tx.raw_tx_id.clone(),
            current_step: 1,
            workflow_data: TransactionWorkflowData {
                alice_transaction: Some(tx),
                charlie_processing: None,
                validation_tasks: Vec::new(),
                alice_completion: None,
                charlie_final_processing: None,
                validator_broadcast: None,
            },
            start_time: Utc::now(),
            last_update: Utc::now(),
        };
        
        // Update consensus state
        let mut state = self.consensus_state.write().await;
        state.active_transactions.insert(workflow_state.tx_id.clone(), workflow_state.clone());
        drop(state);
        
        Ok(workflow_state)
    }

    async fn step2_charlie_processes_transaction(&self, mut workflow_state: TransactionWorkflowState) -> Result<TransactionWorkflowState> {
        log::debug!("Step 2: Charlie processes transaction {}", workflow_state.tx_id);
        
        if let Some(raw_tx) = &workflow_state.workflow_data.alice_transaction {
            // Charlie (local_node) signs the transaction data
            // The data to be signed for ProcessingTransaction could be raw_tx.tx_data or its hash.
            // Let's assume Charlie signs raw_tx.tx_data.
            let data_to_sign_bytes = raw_tx.tx_data.get_bytes_for_signing()
                .map_err(|e| PclError::SerializationError(format!("Failed to serialize tx_data for signing: {}", e)))?;

            let local_keypair = self.local_node_keypair.as_ref();
            let leader_signature = local_keypair.sign_data(&data_to_sign_bytes);
            let leader_signature_hex = hex::encode(leader_signature.to_bytes());

            // Create processing transaction
            let processing_tx = ProcessingTransaction::new(
                raw_tx.raw_tx_id.clone(),
                raw_tx.tx_data.clone(),
                leader_signature_hex, // Real signature
                self.local_node.id.to_string(),
            );
            
            // Add to processing mempool
            let mut mempool = self.mempool.write().await;
            mempool.add_processing_transaction(processing_tx.clone())?;
            drop(mempool);
            
            // Gossip transaction to network
            let mut network = self.network_manager.lock().await;
            network.gossip_transaction(self.local_peer_id.clone(), raw_tx).await?;
            drop(network);
            
            workflow_state.workflow_data.charlie_processing = Some(processing_tx);
            workflow_state.current_step = 2;
            workflow_state.last_update = Utc::now();
        }
        
        Ok(workflow_state)
    }

    async fn step3_leaders_assign_validation_tasks(&self, mut workflow_state: TransactionWorkflowState) -> Result<TransactionWorkflowState> {
        log::debug!("Step 3: Leaders assign validation tasks for tx {}", workflow_state.tx_id);
        
        // Get current leaders
        let leader_election = self.leader_election.read().await;
        let leaders = leader_election.current_leaders.clone();
        drop(leader_election);
        
        // Create validation tasks
        let validation_tasks = vec![
            ValidationTask::new(
                format!("{}_sig_validation", workflow_state.tx_id),
                leaders.get(0).unwrap_or(&"leader1".to_string()).clone(),
                ValidationTaskType::SignatureValidation,
            ),
            ValidationTask::new(
                format!("{}_spend_validation", workflow_state.tx_id),
                leaders.get(1).unwrap_or(&"leader2".to_string()).clone(),
                ValidationTaskType::SpendingPowerValidation,
            ),
            ValidationTask::new(
                format!("{}_timestamp_validation", workflow_state.tx_id),
                leaders.get(2).unwrap_or(&"leader3".to_string()).clone(),
                ValidationTaskType::TimestampValidation,
            ),
        ];
        
        // Add tasks to mempool
        let mut mempool = self.mempool.write().await;
        for task in &validation_tasks {
            mempool.add_validation_task(task.clone())?;
        }
        drop(mempool);
        
        // Send tasks via network
        // We need Alice's PeerId string here.
        // For now, using Alice's Node UUID string as a placeholder for her PeerId string.
        // This requires a mechanism to map Node UUID to actual PeerId for topic subscription.
        let alice_id_str_for_topic = workflow_state.workflow_data.alice_transaction.as_ref()
            .map(|tx| tx.tx_data.user.clone()) // This is Alice's pubkey hex, used as ID in step 1
            .ok_or_else(|| PclError::InvalidState("Missing Alice's transaction for task assignment".to_string()))?;
            // A better way would be to find alice_node again and use a peer_id field from it, if available.
            // For now, assume alice_id_str_for_topic (pubkey hex) is the target for the topic.
            // Or, if nodes subscribe to topics by their UUID:
            // let alice_node_uuid_str = node_registry.nodes.values().find(|n| hex::encode(n.public_key.as_bytes()) == alice_id_str_for_topic).map(|n| n.id.to_string()).unwrap_or_default();


        let mut network = self.network_manager.lock().await;
        for task in &validation_tasks {
            // The 'target_node' for send_validation_task should be Alice's PeerId string.
            // Using alice_id_str_for_topic (which is her pubkey hex, or could be her Node UUID string)
            network.send_validation_task(task, &alice_id_str_for_topic).await?;
        }
        drop(network);
        
        workflow_state.workflow_data.validation_tasks = validation_tasks;
        workflow_state.current_step = 3;
        workflow_state.last_update = Utc::now();
        
        Ok(workflow_state)
    }

    async fn step4_alice_completes_validation_tasks(&self, mut workflow_state: TransactionWorkflowState) -> Result<TransactionWorkflowState> {
        log::debug!("Step 4: Alice completes validation tasks for tx {}", workflow_state.tx_id);
        
        // Complete validation tasks
        let mut validation_engine = self.validation_engine.write().await;
        for task in &workflow_state.workflow_data.validation_tasks {
            let result = ValidationResult {
                task_id: task.task_id.clone(),
                tx_id: workflow_state.tx_id.clone(),
                validation_type: task.task_type.clone(),
                success: true, // Would be actual validation result
                error_message: None,
                completed_at: Utc::now(),
            };
            validation_engine.validation_results.insert(task.task_id.clone(), result);
        }
        drop(validation_engine);
        
        workflow_state.workflow_data.alice_completion = Some(Utc::now());
        workflow_state.current_step = 4;
        workflow_state.last_update = Utc::now();
        
        Ok(workflow_state)
    }

    async fn step5_charlie_processes_validation(&self, mut workflow_state: TransactionWorkflowState) -> Result<TransactionWorkflowState> {
        log::debug!("Step 5: Charlie processes validation for tx {}", workflow_state.tx_id);
        
        // Calculate average timestamp
        let validation_timestamps: Vec<DateTime<Utc>> = workflow_state.workflow_data.validation_tasks
            .iter()
            .filter_map(|task| task.completed_at)
            .collect();
        
        if !validation_timestamps.is_empty() {
            let total_seconds: i64 = validation_timestamps.iter().map(|dt| dt.timestamp()).sum();
            let avg_timestamp = DateTime::from_timestamp(total_seconds / validation_timestamps.len() as i64, 0)
                .unwrap_or(Utc::now());
            
            let mut processor = self.transaction_processor.write().await;
            processor.average_timestamps.insert(workflow_state.tx_id.clone(), avg_timestamp);
            drop(processor);
        }
        
        workflow_state.workflow_data.charlie_final_processing = Some(Utc::now());
        workflow_state.current_step = 5;
        workflow_state.last_update = Utc::now();
        
        Ok(workflow_state)
    }

    async fn step6_validator_broadcasts_and_finalizes(&self, mut workflow_state: TransactionWorkflowState) -> Result<TransactionWorkflowState> {
        log::debug!("Step 6: Validator broadcasts and finalizes tx {}", workflow_state.tx_id);
        
        let alice_tx_data = workflow_state.workflow_data.alice_transaction.as_ref()
            .ok_or_else(|| PclError::InvalidState("Missing Alice's transaction data in workflow".to_string()))?
            .tx_data.clone();

        // Calculate XMBL cubic root from the transaction data
        let xmbl_root = alice_tx_data.calculate_digital_root() as u8;

        // Validator (local_node in this simplified context) signs the finalized transaction details.
        // The data to sign should include key elements like tx_id and xmbl_root.
        // For simplicity, let's sign a concatenation of tx_id and xmbl_root.
        // In a real system, this would be a well-defined structure.
        let data_to_sign_str = format!("{}:{}", workflow_state.tx_id, xmbl_root);
        let data_to_sign_bytes = data_to_sign_str.as_bytes();

        let local_keypair = self.local_node_keypair.as_ref();
        let validator_signature = local_keypair.sign_data(&data_to_sign_bytes);
        let validator_signature_hex = hex::encode(validator_signature.to_bytes());

        // Create finalized transaction
        let finalized_tx = FinalizedTransaction {
            tx_id: workflow_state.tx_id.clone(),
            tx_data: alice_tx_data,
            xmbl_cubic_root: xmbl_root, // Calculated XMBL root
            validator_signature: validator_signature_hex, // Real signature
            finalized_at: Utc::now(),
        };
        
        // Add to transaction mempool
        let mut mempool = self.mempool.write().await;
        mempool.finalize_transaction(workflow_state.tx_id.clone(), finalized_tx.validator_signature.clone())?;
        drop(mempool);
        
        // Store in database
        self.storage_manager.store_finalized_transaction(&finalized_tx)?;
        
        workflow_state.workflow_data.validator_broadcast = Some(Utc::now());
        workflow_state.current_step = 6;
        workflow_state.last_update = Utc::now();
        
        // Remove from active transactions
        let mut state = self.consensus_state.write().await;
        state.active_transactions.remove(&workflow_state.tx_id);
        drop(state);
        
        log::info!("Transaction {} finalized successfully", workflow_state.tx_id);
        Ok(workflow_state)
    }

    // Pulse system implementation
    async fn start_pulse_system(&self) -> Result<()> {
        log::info!("Starting pulse system for node {}", self.local_peer_id);
        let self_clone = self.clone(); // Clone Arc references for the async task

        tokio::spawn(async move {
            // Determine pulse interval from PulseSystem settings
            let pulse_interval_duration = {
                let ps = self_clone.pulse_system.read().await;
                Duration::from_secs(ps.pulse_interval_seconds)
            };
            let mut interval = interval(pulse_interval_duration);

            loop {
                interval.tick().await;
                log::debug!("Node {} sending pulse...", self_clone.local_peer_id);
                if let Err(e) = self_clone.send_pulse().await {
                    log::error!("Error sending pulse for node {}: {}", self_clone.local_peer_id, e);
                }
            }
        });
        
        Ok(())
    }

    // Called periodically by start_pulse_system
    async fn send_pulse(&self) -> Result<()> {
        let pulse_system_rl = self.pulse_system.read().await;

        // Determine which family_id to send to.
        // The current logic uses local_node.id (UUID) to find family.
        // This assumes family_assignments maps Node UUID to Family UUID.
        let family_id_to_pulse = pulse_system_rl.family_assignments.get(&self.local_node.id.to_string()).cloned();

        if let Some(family_id) = family_id_to_pulse {
            drop(pulse_system_rl); // Release read lock before acquiring write lock or network lock

            log::debug!("Node {} attempting to send pulse to family {}", self.local_peer_id, family_id);
            let mut network = self.network_manager.lock().await;
            network.send_pulse(self.local_peer_id.clone(), family_id).await?;
            drop(network);
            
            // Update this node's own last pulse time in its PulseSystem state
            let mut pulse_system_wl = self.pulse_system.write().await;
            pulse_system_wl.last_pulse_time = Utc::now();
            
            // Update or create PulseData for this node within its own PulseSystem state
            // This represents the node's own view of its pulse activity.
            let node_id_key = self.local_node.id.to_string(); // Using Node UUID as key here
            let current_pulse_count = pulse_system_wl.pulse_data.get(&node_id_key)
                                     .map_or(0, |pd| pd.pulse_count);

            let own_pulse_data_entry = pulse_system_wl.pulse_data.entry(node_id_key.clone()).or_insert_with(|| {
                // Initialize if not present
                PulseData {
                    node_id: node_id_key.clone(), // Store own Node UUID
                    family_id, // Family it belongs to / pulsed
                    pulse_count: 0,
                    // These are less relevant for self-entry, more for observed data of others
                    average_response_time_ms: 0.0,
                    uptime_percentage: 100.0, // Own uptime is considered 100% from its perspective
                    last_pulse: Utc::now(),
                }
            });
            
            own_pulse_data_entry.pulse_count = current_pulse_count + 1;
            own_pulse_data_entry.last_pulse = Utc::now();
            // own_pulse_data_entry.uptime_percentage remains 100.0 for self.
            // own_pulse_data_entry.average_response_time_ms is not applicable for self-sent pulse.
            log::debug!("Updated own pulse data for node {}: count {}", node_id_key, own_pulse_data_entry.pulse_count);
        }
        
        Ok(())
    }

    // Leader election implementation
    async fn start_leader_election_cycle(&self) -> Result<()> {
        log::info!("Starting leader election cycle");
        
        let consensus_manager = self.clone();
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(7200)); // 2-hour cycles
            
            loop {
                interval.tick().await;
                
                if let Err(e) = consensus_manager.run_leader_election().await {
                    log::error!("Leader election error: {}", e);
                }
            }
        });
        
        Ok(())
    }

    async fn run_leader_election(&self) -> Result<()> {
        log::info!("Running leader election");
        
        let mut leader_election = self.leader_election.write().await;
        leader_election.election_round += 1;
        leader_election.last_election_time = Utc::now();
        
        // Collect performance data
        let node_registry = self.node_registry.read().await;
        let mut candidates = Vec::new();
        
        for node in node_registry.nodes.values() {
            if node.is_eligible_for_leadership() {
                let performance_score = self.calculate_performance_score(node).await;
                let uptime_score = self.calculate_uptime_score(node).await;
                
                candidates.push(VotingData {
                    candidate_id: node.id.to_string(),
                    votes: 0,
                    performance_score,
                    uptime_score,
                    round: 1,
                });
            }
        }
        drop(node_registry);
        
        // Run 3-round voting
        for round in 1..=3 {
            log::debug!("Leader election round {}", round);
            
            // Simulate voting process
            for candidate in &mut candidates {
                candidate.votes += ((candidate.performance_score + candidate.uptime_score) * 100.0) as u64;
                candidate.round = round;
            }
            
            // Broadcast voting data
            let mut network = self.network_manager.lock().await;
            for candidate in &candidates {
                network.broadcast_leader_election(
                    &format!("election_{}", leader_election.election_round),
                    &candidate.candidate_id,
                    candidate.votes,
                    round,
                ).await?;
            }
            drop(network);
            
            // Wait between rounds
            sleep(Duration::from_secs(30)).await;
        }
        
        // Select top performers as leaders
        candidates.sort_by(|a, b| b.votes.cmp(&a.votes));
        leader_election.current_leaders = candidates.into_iter()
            .take(3)
            .map(|c| c.candidate_id)
            .collect();
        
        leader_election.voting_data.clear();
        
        log::info!("Leader election completed. New leaders: {:?}", leader_election.current_leaders);
        Ok(())
    }

    async fn calculate_performance_score(&self, node: &Node) -> f64 {
        // Performance can be based on average response time. Lower is better.
        // We need a way to normalize this into a score from 0.0 to 1.0.
        // Example: Score = 1.0 - (avg_response_ms / max_observed_avg_response_ms_cap)
        // Or, if very low response times are common, use a fixed scale.
        let node_uuid_str = node.id.to_string();
        let mempool = self.mempool.read().await;

        if let Some(avg_rt) = mempool.get_node_average_response_time(&node_uuid_str) {
            // Normalize: e.g., cap at 1000ms. Response times < 50ms get high score.
            // (1 - (min(avg_rt, 1000.0) / 1000.0)) should give a score where lower RT is better.
            // Let's try: if avg_rt < 50ms -> 1.0, if 500ms -> 0.5, if 1000ms -> 0.0
            // Score = 1.0 - (avg_rt / 1000.0), clamped to [0.0, 1.0]
            let score = 1.0 - (avg_rt / 1000.0);
            score.max(0.0).min(1.0)
        } else {
            0.1 // Default low score if no response time data
        }
    }

    async fn calculate_uptime_score(&self, node: &Node) -> f64 {
        // Query UptimeMempool using node's application-level UUID string
        let node_uuid_str = node.id.to_string();
        let mempool = self.mempool.read().await;
        let uptime_percentage = mempool.calculate_node_uptime_percentage(&node_uuid_str);
        // Ensure uptime_percentage is used correctly (e.g., already 0-100 or needs scaling)
        // The calculate_node_uptime_percentage returns 0.0 to 100.0. So divide by 100 for score.
        uptime_percentage / 100.0
    }

    // Background processing tasks
    async fn start_transaction_processing(&self) -> Result<()> {
        log::info!("Starting transaction processing");
        
        let consensus_manager = self.clone();
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(5));
            
            loop {
                interval.tick().await;
                
                if let Err(e) = consensus_manager.process_pending_transactions().await {
                    log::error!("Transaction processing error: {}", e);
                }
            }
        });
        
        Ok(())
    }

    async fn process_pending_transactions(&self) -> Result<()> {
        let mut processor = self.transaction_processor.write().await;
        let queue = processor.processing_queue.clone();
        processor.processing_queue.clear();
        drop(processor);
        
        for tx in queue {
            if let Err(e) = self.process_transaction_workflow(tx).await {
                log::error!("Failed to process transaction: {}", e);
            }
        }
        
        Ok(())
    }

    async fn start_validation_engine(&self) -> Result<()> {
        log::info!("Starting validation engine");
        
        let consensus_manager = self.clone();
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(2));
            
            loop {
                interval.tick().await;
                
                if let Err(e) = consensus_manager.process_validation_tasks().await {
                    log::error!("Validation engine error: {}", e);
                }
            }
        });
        
        Ok(())
    }

    async fn process_validation_tasks(&self) -> Result<()> {
        let mut validation_engine = self.validation_engine.write().await;
        let active_tasks: Vec<ValidationTask> = validation_engine.active_tasks.values().cloned().collect();
        
        for mut task in active_tasks {
            // Simulate validation completion
            if !task.complete && task.assigned_at < Utc::now() - chrono::Duration::seconds(10) {
                task.complete();
                
                let result = ValidationResult {
                    task_id: task.task_id.clone(),
                    tx_id: task.task_id.split('_').next().unwrap_or("unknown").to_string(),
                    validation_type: task.task_type.clone(),
                    success: true,
                    error_message: None,
                    completed_at: Utc::now(),
                };
                
                let task_id = task.task_id.clone();
                validation_engine.completed_tasks.insert(task_id.clone(), task);
                validation_engine.validation_results.insert(result.task_id.clone(), result);
                validation_engine.active_tasks.remove(&task_id);
            }
        }
        
        Ok(())
    }

    // System status and monitoring
    pub async fn get_system_status(&self) -> Result<SystemStatus> {
        let state = self.consensus_state.read().await;
        let mempool = self.mempool.read().await;
        let pulse_system = self.pulse_system.read().await;
        let leader_election = self.leader_election.read().await;
        
        let status = SystemStatus {
            consensus_phase: state.current_phase.clone(),
            active_transactions: state.active_transactions.len(),
            current_leaders: leader_election.current_leaders.clone(),
            mempool_stats: mempool.get_mempool_stats(),
            pulse_data: pulse_system.pulse_data.values().cloned().collect(),
            system_load: state.system_load,
            network_health: state.network_health,
        };
        
        Ok(status)
    }

    // Main handler for messages received from the network
    pub async fn handle_network_message(&self, message: NetworkMessage) -> Result<()> {
        match message {
            NetworkMessage::Pulse(pulse_msg) => self.handle_pulse_message(pulse_msg).await,
            NetworkMessage::PulseResponse(pulse_response_msg) => self.handle_pulse_response_message(pulse_response_msg).await,
            NetworkMessage::TransactionGossip(tx_gossip_msg) => self.handle_transaction_gossip(tx_gossip_msg).await,
            NetworkMessage::ValidationTask(validation_task_msg) => self.handle_validation_task_message(validation_task_msg).await,
            NetworkMessage::LeaderElection(leader_election_msg) => self.handle_leader_election_message(leader_election_msg).await,
            NetworkMessage::UptimeData(uptime_data_msg) => self.handle_uptime_data_message(uptime_data_msg).await,
            // Add other message types as needed
        }
    }

    async fn handle_pulse_message(&self, msg: PulseMessage) -> Result<()> {
        log::debug!("Received PulseMessage from Node UUID {} (PeerId {}) for family {}", msg.sender_node_uuid, msg.sender_peer_id, msg.family_id);

        // 1. Record the received pulse in UptimeMempool
        let mut mempool = self.mempool.write().await;
        mempool.record_received_pulse(msg.sender_node_uuid.clone(), msg.family_id, msg.timestamp)?;
        drop(mempool);

        // 2. Send a PulseResponseMessage back to the sender
        //    We need the sender's PeerId (msg.sender_peer_id) to target the response.
        //    The response time is calculated by the recipient of the response.
        //    Here, we are just acknowledging the pulse. The actual response time will be
        //    calculated by msg.sender_peer_id when it receives our response.
        //    For now, let's set response_time_ms to 0, as it's not used by the receiver of PulseResponseMessage in this way.
        //    The critical part is that the original sender measures the RTT.

        // The node responding is self.local_node.id (UUID) and self.local_peer_id.
        // The target for the response is msg.sender_peer_id.
        let response_time_for_this_leg: u64 = 10; // Simulated processing time before responding

        let mut network = self.network_manager.lock().await;
        network.send_pulse_response(
            self.local_node.id.to_string(), // Our Node UUID
            &msg.sender_peer_id,            // Target PeerID for the response
            &msg.pulse_id,
            response_time_for_this_leg
        ).await?;
        log::debug!("Sent PulseResponse for pulse_id {} to PeerId {}", msg.pulse_id, msg.sender_peer_id);
        Ok(())
    }

    async fn handle_pulse_response_message(&self, msg: PulseResponseMessage) -> Result<()> {
        log::debug!("Received PulseResponseMessage from Node UUID {} (PeerId {}) for pulse_id {}: rt {}ms", msg.responder_node_uuid, msg.responder_peer_id, msg.pulse_id, msg.response_time_ms);

        // Record this response time in UptimeMempool for the responder_node_uuid
        let mut mempool = self.mempool.write().await;
        mempool.record_received_pulse_response(
            msg.responder_node_uuid.clone(),
            msg.pulse_id.clone(),
            msg.response_time_ms,
            msg.timestamp,
        )?;
        Ok(())
    }

    async fn handle_transaction_gossip(&self, msg: TransactionGossipMessage) -> Result<()> {
        log::info!("Received TransactionGossip for tx_id: {}", msg.tx_id);
        // TODO: Add to mempool, potentially trigger workflow if this node is Charlie
        // For now, just add to raw_tx_mempool if not already present
        let mut mempool = self.mempool.write().await;
        if mempool.raw_tx.get_transaction(&msg.tx_id).is_none() {
            log::debug!("Adding gossiped transaction {} to mempool", msg.tx_id);
            mempool.add_raw_transaction(msg.raw_transaction)?;
            // Potentially, if this node is the designated leader (Charlie for this tx),
            // it could start step2_charlie_processes_transaction or parts of it.
            // This requires knowing the leader for a given tx.
        } else {
            log::debug!("Gossiped transaction {} already in mempool", msg.tx_id);
        }
        Ok(())
    }

    async fn handle_validation_task_message(&self, msg: ValidationTaskMessage) -> Result<()> {
        log::info!("Received ValidationTaskMessage for task_id: {} targeted at {}", msg.task_id, msg.target_node);
        // msg.target_node is expected to be this node's PeerId string or Node UUID string based on topic subscription.
        // If this node is indeed the target (Alice), add to its pending tasks.
        // This requires ValidationEngine to store tasks by Node ID.
        // For now, let's assume if we receive it, it's for us.
        let mut validation_engine = self.validation_engine.write().await;
        if validation_engine.active_tasks.contains_key(&msg.task_id) || validation_engine.completed_tasks.contains_key(&msg.task_id) {
            log::debug!("Validation task {} already known.", msg.task_id);
            return Ok(());
        }
        log::debug!("Adding validation task {} to active_tasks for this node.", msg.task_id);
        validation_engine.active_tasks.insert(msg.task_id.clone(), msg.task);
        // TODO: Alice would then process this task and send a ValidationCompletionMessage
        Ok(())
    }

    async fn handle_leader_election_message(&self, msg: LeaderElectionMessage) -> Result<()> {
        log::info!("Received LeaderElectionMessage for election_id: {}, candidate: {}, votes: {}", msg.election_id, msg.candidate_id, msg.votes);
        // TODO: Aggregate votes during leader election rounds.
        // This requires LeaderElectionManager to store incoming votes.
        let mut leader_election_manager = self.leader_election.write().await;
        // Assuming msg.candidate_id is the Node UUID string
        let vote_data = leader_election_manager.voting_data
            .entry(msg.candidate_id.clone())
            .or_insert_with(|| VotingData {
                candidate_id: msg.candidate_id.clone(),
                votes: 0,
                performance_score: 0.0, // This would ideally be looked up or sent with vote
                uptime_score: 0.0,    // This would ideally be looked up or sent with vote
                round: msg.round,
            });

        // Simplistic: just add votes. Real voting needs rounds and more complex logic.
        // Also, ensure votes are for the current round.
        if vote_data.round == msg.round || leader_election_manager.election_round == 0 { // Allow first votes
             if vote_data.round != msg.round { // New round for this candidate
                vote_data.votes = 0;
                vote_data.round = msg.round;
            }
            vote_data.votes += msg.votes;
            log::debug!("Aggregated votes for {}: total {}, round {}", msg.candidate_id, vote_data.votes, msg.round);
        } else {
            log::warn!("Received vote for candidate {} for round {} but current/candidate round is different (LEM round {}, candidate data round {}). Ignoring.",
                msg.candidate_id, msg.round, leader_election_manager.election_round, vote_data.round
            );
        }
        Ok(())
    }

    async fn handle_uptime_data_message(&self, msg: UptimeMessage) -> Result<()> {
        log::info!("Received UptimeDataMessage from node_id: {} ({}%)", msg.node_id, msg.uptime_percentage);
        // TODO: Potentially update UptimeMempool if this data is considered authoritative
        // For now, our UptimeMempool is based on direct observation of pulses/responses.
        // This message type might be for nodes broadcasting their self-perceived status.
        Ok(())
    }

}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStatus {
    pub consensus_phase: ConsensusPhase,
    pub active_transactions: usize,
    pub current_leaders: Vec<String>,
    pub mempool_stats: crate::mempool::MempoolStats,
    pub pulse_data: Vec<PulseData>,
    pub system_load: f64,
    pub network_health: f64,
}

// Implementation of Default and New traits for supporting structs
impl LeaderElectionManager {
    pub fn new() -> Self {
        Self {
            current_leaders: Vec::new(),
            election_round: 0,
            last_election_time: Utc::now(),
            voting_data: HashMap::new(),
            broadcasting_cycle: Arc::new(RwLock::new(BroadcastingCycle {
                cycle_start: Utc::now(),
                cycle_duration_hours: 2,
                current_leaders: Vec::new(),
            })),
        }
    }
}

impl PulseSystem {
    pub fn new() -> Self {
        Self {
            pulse_interval_seconds: 20,
            family_assignments: HashMap::new(),
            pulse_data: HashMap::new(),
            response_times: HashMap::new(),
            last_pulse_time: Utc::now(),
        }
    }
}

impl TransactionProcessor {
    pub fn new() -> Self {
        Self {
            processing_queue: Vec::new(),
            validation_assignments: HashMap::new(),
            average_timestamps: HashMap::new(),
            leader_signatures: HashMap::new(),
        }
    }
}

impl ValidationEngine {
    pub fn new() -> Self {
        Self {
            active_tasks: HashMap::new(),
            completed_tasks: HashMap::new(),
            validation_results: HashMap::new(),
        }
    }
}

impl ConsensusState {
    pub fn new() -> Self {
        Self {
            current_phase: ConsensusPhase::Initialization,
            active_transactions: HashMap::new(),
            leader_performance: HashMap::new(),
            system_load: 0.0,
            network_health: 100.0,
        }
    }
}

// Make ConsensusManager cloneable for background tasks
impl Clone for ConsensusManager {
    fn clone(&self) -> Self {
        Self {
            node_registry: self.node_registry.clone(),
            mempool: self.mempool.clone(),
            network_manager: self.network_manager.clone(),
            storage_manager: self.storage_manager.clone(),
            local_node: self.local_node.clone(),
            local_peer_id: self.local_peer_id.clone(),
            local_node_keypair: self.local_node_keypair.clone(), // Added for signing
            leader_election: self.leader_election.clone(),
            pulse_system: self.pulse_system.clone(),
            transaction_processor: self.transaction_processor.clone(),
            validation_engine: self.validation_engine.clone(),
            consensus_state: self.consensus_state.clone(),
        }
    }
}

// Safety: NetworkManager is Send + Sync due to Arc<Mutex<>> wrapper
unsafe impl Send for ConsensusManager {}
unsafe impl Sync for ConsensusManager {}

// Serialization support for ConsensusPhase
impl Serialize for ConsensusPhase {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            ConsensusPhase::Initialization => serializer.serialize_str("initialization"),
            ConsensusPhase::NormalOperation => serializer.serialize_str("normal_operation"),
            ConsensusPhase::LeaderElection => serializer.serialize_str("leader_election"),
            ConsensusPhase::NetworkPartition => serializer.serialize_str("network_partition"),
            ConsensusPhase::Recovery => serializer.serialize_str("recovery"),
        }
    }
}

impl<'de> Deserialize<'de> for ConsensusPhase {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        match String::deserialize(deserializer)?.as_str() {
            "initialization" => Ok(ConsensusPhase::Initialization),
            "normal_operation" => Ok(ConsensusPhase::NormalOperation),
            "leader_election" => Ok(ConsensusPhase::LeaderElection),
            "network_partition" => Ok(ConsensusPhase::NetworkPartition),
            "recovery" => Ok(ConsensusPhase::Recovery),
            _ => Err(serde::de::Error::custom("Invalid consensus phase")),
        }
    }
} 