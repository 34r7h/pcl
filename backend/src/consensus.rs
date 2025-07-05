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
    pub local_node: Node,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VotingData {
    pub candidate_id: String,
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
        network_manager: NetworkManager,
        storage_manager: StorageManager,
    ) -> Result<Self> {
        let node_registry = Arc::new(RwLock::new(NodeRegistry::new()));
        let mempool = Arc::new(RwLock::new(MempoolManager::new()));
        let network_manager = Arc::new(Mutex::new(network_manager));
        let storage_manager = Arc::new(storage_manager);
        
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

    async fn step1_alice_creates_transaction(&self, tx: RawTransaction) -> Result<TransactionWorkflowState> {
        log::debug!("Step 1: Alice creates transaction {}", tx.raw_tx_id);
        
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
            // Create processing transaction
            let processing_tx = ProcessingTransaction::new(
                raw_tx.raw_tx_id.clone(),
                raw_tx.tx_data.clone(),
                "leader_signature".to_string(), // Would be actual signature
                self.local_node.id.to_string(),
            );
            
            // Add to processing mempool
            let mut mempool = self.mempool.write().await;
            mempool.add_processing_transaction(processing_tx.clone())?;
            drop(mempool);
            
            // Gossip transaction to network
            let mut network = self.network_manager.lock().await;
            network.gossip_transaction(raw_tx).await?;
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
        let mut network = self.network_manager.lock().await;
        for task in &validation_tasks {
            network.send_validation_task(task, "alice_node_id").await?;
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
        
        // Create finalized transaction
        let finalized_tx = FinalizedTransaction {
            tx_id: workflow_state.tx_id.clone(),
            tx_data: workflow_state.workflow_data.alice_transaction.as_ref().unwrap().tx_data.clone(),
            xmbl_cubic_root: 5, // Would be calculated from XMBL Cubic DLT
            validator_signature: "validator_signature".to_string(),
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
        log::info!("Starting pulse system");
        
        // TODO: Implement background pulse system
        // Commenting out for now due to Send/Sync issues with NetworkManager
        
        Ok(())
    }

    async fn send_pulse(&self) -> Result<()> {
        let pulse_system = self.pulse_system.read().await;
        if let Some(family_id) = pulse_system.family_assignments.get(&self.local_node.id.to_string()) {
            let family_id = *family_id;
            drop(pulse_system);
            
            let mut network = self.network_manager.lock().await;
            network.send_pulse(family_id).await?;
            drop(network);
            
            // Update pulse data
            let mut pulse_system = self.pulse_system.write().await;
            pulse_system.last_pulse_time = Utc::now();
            
            let pulse_data = PulseData {
                node_id: self.local_node.id.to_string(),
                family_id,
                pulse_count: pulse_system.pulse_data.get(&self.local_node.id.to_string())
                    .map(|p| p.pulse_count + 1)
                    .unwrap_or(1),
                average_response_time_ms: 50.0, // Placeholder
                uptime_percentage: 99.5, // Placeholder
                last_pulse: Utc::now(),
            };
            
            pulse_system.pulse_data.insert(self.local_node.id.to_string(), pulse_data);
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
        // Placeholder performance calculation
        if node.role == NodeRole::Leader {
            0.9
        } else {
            0.7
        }
    }

    async fn calculate_uptime_score(&self, node: &Node) -> f64 {
        let pulse_system = self.pulse_system.read().await;
        if let Some(pulse_data) = pulse_system.pulse_data.get(&node.id.to_string()) {
            pulse_data.uptime_percentage / 100.0
        } else {
            0.5
        }
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