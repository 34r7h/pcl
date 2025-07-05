use crate::data_structures::{
    TransactionData, RawTransactionEntry, ValidationTaskItem, ProcessingTransactionEntry,
    FinalizedTransactionEntry, UptimeEntry, RawTxId, TxId, NodeId, UtxoId,
};
use crate::db::AllMempoolDbs;
use crate::network::{ConsensusMessage, NetworkManager}; // NetworkManager might be passed or a sender channel to it
use chrono::Utc;
use log::{info, warn, error};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc; // For shared state like DBs
use tokio::sync::Mutex; // For mutable shared state if NetworkManager is shared

// Configuration for the consensus protocol (can be loaded from a file or env vars)
pub struct ConsensusConfig {
    pub required_validation_timestamps: usize,
    pub leader_election_interval_hours: i64,
    pub node_pulse_interval_seconds: i64,
    pub node_offline_threshold_seconds: i64,
    // ... other config parameters
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        ConsensusConfig {
            required_validation_timestamps: 3, // Example value
            leader_election_interval_hours: 2,
            node_pulse_interval_seconds: 20,
            node_offline_threshold_seconds: 60,
        }
    }
}

pub struct ConsensusNode {
    node_id: NodeId, // This node's ID
    dbs: Arc<AllMempoolDbs>,
    // network_manager: Arc<Mutex<NetworkManager>>, // If NetworkManager needs to be shared and mutated
    // For sending messages, a channel sender might be better than sharing NetworkManager directly
    network_sender: tokio::sync::mpsc::UnboundedSender<ConsensusMessage>, // To send messages out via NetworkManager
    config: ConsensusConfig,
    // current_leaders: Vec<NodeId>, // Updated via leader election
    // is_leader: bool, // Derived from current_leaders and node_id
}

impl ConsensusNode {
    pub fn new(
        node_id: NodeId,
        dbs: Arc<AllMempoolDbs>,
        network_sender: tokio::sync::mpsc::UnboundedSender<ConsensusMessage>,
        config: ConsensusConfig,
    ) -> Self {
        ConsensusNode {
            node_id,
            dbs,
            network_sender,
            config,
        }
    }

    fn calculate_raw_tx_id(tx_data: &TransactionData) -> RawTxId {
        let mut hasher = Sha256::new();
        // Ensure a consistent serialization for hashing
        // Signature should be None or consistent if part of raw_tx_id calculation
        let mut tx_data_for_hash = tx_data.clone();
        tx_data_for_hash.sig = None; // Signature is on the content, not part of this initial ID usually

        if let Ok(serialized_tx) = serde_json::to_string(&tx_data_for_hash) {
            hasher.update(serialized_tx);
            format!("{:x}", hasher.finalize())
        } else {
            // Fallback or error, this should not happen with valid TransactionData
            "invalid_tx_data_hash".to_string()
        }
    }

    fn calculate_final_tx_id(avg_timestamp: i64, tx_data: &TransactionData) -> TxId {
        let mut hasher = Sha256::new();
        let data_to_hash = format!("{}:{:?}", avg_timestamp, tx_data); // Simple concatenation
        hasher.update(data_to_hash);
        format!("{:x}", hasher.finalize())
    }


    // Entry point for a new transaction from a user (e.g. Alice)
    pub async fn handle_new_transaction_request(&self, tx_data: TransactionData) -> Result<RawTxId, String> {
        info!("Node {} received new transaction request: {:?}", self.node_id, tx_data);
        // TODO: Basic validation of tx_data (e.g., fees, stake, signature if provided)

        let raw_tx_id = Self::calculate_raw_tx_id(&tx_data);
        let current_timestamp = Utc::now().timestamp_millis();

        // Step 2 (partial): Create raw_tx_mempool entry
        let raw_tx_entry = RawTransactionEntry {
            tx_data: tx_data.clone(),
            validation_timestamps: Vec::new(),
            validation_tasks: Vec::new(), // Tasks will be added by other leaders
            tx_timestamp: current_timestamp,
        };

        // Store in this leader's raw_tx_mempool (NodeId -> RawTxId -> RawTxEntry)
        // The README implies raw_tx_mempool is { charlie_id: { raw_tx_id: {...} } }
        // So, the primary key in RocksDB might be (NodeId, RawTxId) or NodeId, and value is HashMap<RawTxId, RawTxEntry>
        // Using the latter for the db.rs structure (String key, String value for map)

        // Get current map for this node_id, or create new
        let mut node_txs: HashMap<RawTxId, RawTransactionEntry> = self
            .dbs
            .raw_tx_mempool_db
            .get(&self.node_id)
            .map_err(|e| e.to_string())?
            .and_then(|json_str| serde_json::from_str(&json_str).ok())
            .unwrap_or_default();

        node_txs.insert(raw_tx_id.clone(), raw_tx_entry.clone());
        let node_txs_json = serde_json::to_string(&node_txs).map_err(|e| e.to_string())?;
        self.dbs.raw_tx_mempool_db.put(&self.node_id, &node_txs_json).map_err(|e| e.to_string())?;
        info!("Node {} added {} to its raw_tx_mempool", self.node_id, raw_tx_id);

        // Add to validation_tasks_mempool (RawTxId -> Vec<ValidationTaskItem>)
        // Initially, this might be empty or represent that tasks are needed.
        // "Charlie also adds Alice's raw_tx_id to the validation_tasks_mempool."
        // Let's store an empty Vec, signifying it's pending tasks.
        let initial_tasks: Vec<ValidationTaskItem> = Vec::new();
        let tasks_json = serde_json::to_string(&initial_tasks).map_err(|e| e.to_string())?;
        self.dbs.validation_tasks_mempool_db.put(&raw_tx_id, &tasks_json).map_err(|e| e.to_string())?;
        info!("Node {} added {} to validation_tasks_mempool", self.node_id, raw_tx_id);

        // Lock UTXOs
        for utxo_id in tx_data.from.keys() {
            self.dbs.locked_utxo_mempool_db.put(utxo_id, &current_timestamp).map_err(|e| e.to_string())?;
            info!("Node {} locked UTXO {}", self.node_id, utxo_id);
        }

        // Gossip to other leaders (Step 2)
        let gossip_message = ConsensusMessage::RawTransactionShare {
            from_node_id: self.node_id.clone(),
            raw_tx_id: raw_tx_id.clone(),
            raw_tx_entry, // Contains tx_data
        };
        self.network_sender.send(gossip_message).map_err(|e| e.to_string())?;
        info!("Node {} gossiped RawTransactionShare for {}", self.node_id, raw_tx_id);

        Ok(raw_tx_id)
    }

    // Handles incoming messages from the network
    pub async fn process_network_message(&self, message: ConsensusMessage) -> Result<(), String> {
        info!("Node {} processing network message: {:?}", self.node_id, message);
        match message {
            ConsensusMessage::RawTransactionShare { from_node_id, raw_tx_id, raw_tx_entry } => {
                // Another leader (from_node_id) shared a raw transaction.
                // This node (self.node_id, also a leader) should:
                // 1. Store it in its own raw_tx_mempool under `from_node_id`.
                // 2. Add `raw_tx_id` to its `validation_tasks_mempool`.
                // 3. Lock UTXOs.
                // 4. Potentially generate validation tasks for the user if this leader is responsible. (README Step 3 implies this)
                //    "The other leaders send Charlie validation tasks for Alice to complete."
                //    This means this leader (if not Charlie) should generate tasks and send them to Charlie.
                //    Or, it means this leader *receives* tasks from Charlie to give to Alice if Alice is connected to this leader.
                //    Let's assume: if I'm a leader, and I get a RawTransactionShare, I should:
                //    a. Store it.
                //    b. If I'm *not* the original leader (Charlie/from_node_id), I might propose tasks to `from_node_id`.
                //       This part of the flow ("other leaders send Charlie validation tasks") needs clarification.
                //       For now, let's just store and lock. Task generation/assignment is complex.

                if from_node_id == self.node_id { // Message about my own transaction, already handled or loopback
                    return Ok(());
                }

                // Store in raw_tx_mempool under the original leader's ID
                let mut origin_leader_txs: HashMap<RawTxId, RawTransactionEntry> = self
                    .dbs
                    .raw_tx_mempool_db
                    .get(&from_node_id)
                    .map_err(|e| e.to_string())?
                    .and_then(|json_str| serde_json::from_str(&json_str).ok())
                    .unwrap_or_default();

                if !origin_leader_txs.contains_key(&raw_tx_id) {
                    origin_leader_txs.insert(raw_tx_id.clone(), raw_tx_entry.clone());
                    let json_val = serde_json::to_string(&origin_leader_txs).map_err(|e| e.to_string())?;
                    self.dbs.raw_tx_mempool_db.put(&from_node_id, &json_val).map_err(|e| e.to_string())?;
                    info!("Node {} stored RawTransactionShare from {} for {}", self.node_id, from_node_id, raw_tx_id);

                    // Add to this node's validation_tasks_mempool if not already there
                    if self.dbs.validation_tasks_mempool_db.get(&raw_tx_id).map_err(|e|e.to_string())?.is_none() {
                        let initial_tasks: Vec<ValidationTaskItem> = Vec::new(); // Signifies tasks are pending/needed
                        let tasks_json = serde_json::to_string(&initial_tasks).map_err(|e| e.to_string())?;
                        self.dbs.validation_tasks_mempool_db.put(&raw_tx_id, &tasks_json).map_err(|e| e.to_string())?;
                    }

                    // Lock UTXOs
                    let current_timestamp = Utc::now().timestamp_millis();
                    for utxo_id in raw_tx_entry.tx_data.from.keys() {
                        if self.dbs.locked_utxo_mempool_db.get(utxo_id).map_err(|e|e.to_string())?.is_none() {
                             self.dbs.locked_utxo_mempool_db.put(utxo_id, &current_timestamp).map_err(|e| e.to_string())?;
                             info!("Node {} (on behalf of {}) locked UTXO {}", self.node_id, from_node_id, utxo_id);
                        }
                    }
                    // TODO: Implement logic for "other leaders send Charlie validation tasks".
                    // This might involve this leader generating some tasks and sending them to `from_node_id`.
                    // Or this leader preparing to validate if tasks are assigned to it.
                }
            }
            ConsensusMessage::ValidationTaskSubmission { from_user_or_validator_id, raw_tx_id, completed_tasks } => {
                // This message comes from a user/validator to a leader (self.node_id).
                // The leader (self.node_id) needs to:
                // 1. Find the raw_tx_entry (it should be in its own raw_tx_mempool, under its own node_id as key).
                // 2. Verify the completed tasks (e.g., check signatures, task validity).
                // 3. Update the `validation_tasks` and `validation_timestamps` in the RawTransactionEntry.
                // 4. If all required tasks/timestamps are met, proceed to Step 5 (averaging, processing_tx_mempool).

                let mut node_txs: HashMap<RawTxId, RawTransactionEntry> = self
                    .dbs
                    .raw_tx_mempool_db
                    .get(&self.node_id) // Assuming this leader is the one who initiated (like Charlie)
                    .map_err(|e| e.to_string())?
                    .and_then(|json_str| serde_json::from_str(&json_str).ok())
                    .ok_or_else(|| format!("No raw_tx_mempool found for this leader {}", self.node_id))?;

                let raw_tx_entry = node_txs.get_mut(&raw_tx_id)
                    .ok_or_else(|| format!("Raw tx {} not found in {}'s mempool", raw_tx_id, self.node_id))?;

                info!("Processing task submission for {} from {}", raw_tx_id, from_user_or_validator_id);
                let current_timestamp = Utc::now().timestamp_millis();

                for completed_task in completed_tasks {
                    // Find the corresponding task in raw_tx_entry.validation_tasks and mark as complete.
                    // The task_id should be unique.
                    if let Some(task_in_mempool) = raw_tx_entry.validation_tasks.iter_mut()
                        .find(|t| t.task_id == completed_task.task_id && t.assigned_by_leader_id == self.node_id) { // Ensure task was assigned by this leader
                        if completed_task.complete { // TODO: Add actual validation logic here
                            task_in_mempool.complete = true;
                            raw_tx_entry.validation_timestamps.push(current_timestamp); // Add a timestamp for this validation
                            info!("Task {} for {} marked complete.", completed_task.task_id, raw_tx_id);
                        } else {
                            warn!("Submitted task {} for {} was not marked complete by validator.", completed_task.task_id, raw_tx_id);
                        }
                    } else {
                        warn!("Received submission for unknown or unassigned task ID {} for tx {}", completed_task.task_id, raw_tx_id);
                    }
                }

                // Persist changes to raw_tx_entry
                let node_txs_json = serde_json::to_string(&node_txs).map_err(|e| e.to_string())?;
                self.dbs.raw_tx_mempool_db.put(&self.node_id, &node_txs_json).map_err(|e| e.to_string())?;

                // Check if required number of validations are met (README Step 5)
                if raw_tx_entry.validation_timestamps.len() >= self.config.required_validation_timestamps {
                    info!("Sufficient validations for {}. Proceeding to processing.", raw_tx_id);
                    // Remove from raw_tx_mempool (for this leader)
                    node_txs.remove(&raw_tx_id);
                    let updated_node_txs_json = serde_json::to_string(&node_txs).map_err(|e| e.to_string())?;
                    self.dbs.raw_tx_mempool_db.put(&self.node_id, &updated_node_txs_json).map_err(|e| e.to_string())?;

                    // Remove from validation_tasks_mempool
                    self.dbs.validation_tasks_mempool_db.delete(&raw_tx_id).map_err(|e|e.to_string())?;

                    // Average timestamps
                    let avg_timestamp = if !raw_tx_entry.validation_timestamps.is_empty() {
                        raw_tx_entry.validation_timestamps.iter().sum::<i64>() / raw_tx_entry.validation_timestamps.len() as i64
                    } else {
                        Utc::now().timestamp_millis() // Fallback, should not happen if validations > 0
                    };

                    let final_tx_id = Self::calculate_final_tx_id(avg_timestamp, &raw_tx_entry.tx_data);
                    let mut tx_data_with_avg_ts = HashMap::new();
                    tx_data_with_avg_ts.insert(avg_timestamp, raw_tx_entry.tx_data.clone());

                    // TODO: Sign the {timestamp: tx_data} - requires leader's private key
                    let leader_signature = format!("signature_by_{}_for_{}", self.node_id, final_tx_id); // Placeholder

                    let processing_entry = ProcessingTransactionEntry {
                        tx_data_with_avg_ts,
                        sig: leader_signature,
                        leader_id: self.node_id.clone(),
                    };

                    let processing_entry_json = serde_json::to_string(&processing_entry).map_err(|e| e.to_string())?;
                    self.dbs.processing_tx_mempool_db.put(&final_tx_id, &processing_entry_json).map_err(|e| e.to_string())?;
                    info!("Node {} moved {} to processing_tx_mempool as {}", self.node_id, raw_tx_id, final_tx_id);

                    // Gossip ProcessingTransactionShare (Step 6)
                    let gossip_msg = ConsensusMessage::ProcessingTransactionShare {
                        from_node_id: self.node_id.clone(),
                        tx_id: final_tx_id.clone(),
                        processing_tx_entry: processing_entry,
                    };
                    self.network_sender.send(gossip_msg).map_err(|e| e.to_string())?;
                    info!("Node {} gossiped ProcessingTransactionShare for {}", self.node_id, final_tx_id);

                    // TODO: Step 5 also mentions "Another task type is put in the validation_task_mempool to send to validators --
                    // check Charlie's math from averaging timestamps and hash the {timestamp: tx_data} value to get Alice's tx_id."
                    // This implies a new set of validation tasks for the *processing* transaction.
                }

            }
            ConsensusMessage::ProcessingTransactionShare { from_node_id, tx_id, processing_tx_entry } => {
                // Another leader shared a processed transaction.
                // This node (leader) should:
                // 1. Add to its processing_tx_mempool.
                // 2. (README Step 6) "add/check that finality validation tasks for their chain of choice"
                // 3. "remove the entry in their raw_tx_mempool and associated validation_tasks_mempool entries"
                //    (This implies finding the original raw_tx_id, which is not directly in ProcessingTransactionShare)
                //    This might require a lookup or the RawTxId to be part of ProcessingTransactionShare message.
                //    For now, we'll assume we only handle the processing_tx_mempool update.

                if from_node_id == self.node_id { return Ok(()); } // Already handled by self

                if self.dbs.processing_tx_mempool_db.get(&tx_id).map_err(|e|e.to_string())?.is_none() {
                    let processing_entry_json = serde_json::to_string(&processing_tx_entry).map_err(|e| e.to_string())?;
                    self.dbs.processing_tx_mempool_db.put(&tx_id, &processing_entry_json).map_err(|e| e.to_string())?;
                    info!("Node {} stored ProcessingTransactionShare from {} for tx_id {}", self.node_id, from_node_id, tx_id);

                    // TODO: Implement step 6 logic: remove from raw_tx_mempool etc. This requires linking tx_id back to raw_tx_id.
                    // TODO: Implement finality validation tasks for the chosen DLT.
                }
            }
            // ... handle other message types like InvalidateTransaction, UptimePulse, etc.
            _ => {
                warn!("Node {} received unhandled message type: {:?}", self.node_id, message);
            }
        }
        Ok(())
    }

    // TODO: Implement leader election logic (pulsing, uptime calculation, voting)
    // TODO: Implement transaction finalization logic (e.g., for XMBL DLT)
    // TODO: Implement handling of invalidations
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::AllMempoolDbs;
    use std::fs;
    use tempfile::tempdir;
    use crate::data_structures::Mempools; // In-memory version for simple tests

    fn setup_test_environment(node_id: &str) -> (ConsensusNode, tokio::sync::mpsc::UnboundedReceiver<ConsensusMessage>, Arc<AllMempoolDbs>) {
        // Create a temporary directory for RocksDB data for this test run
        let temp_dir = tempdir().unwrap();
        // Override DB_BASE_PATH for tests IF db.rs used a configurable base path.
        // Since db.rs hardcodes "./db_data/", tests involving AllMempoolDbs will create this dir.
        // We should ensure it's cleaned up or unique per test. For now, let it create.
        // For isolated tests, it's better if AllMempoolDbs can take a base path.

        // Clean up any previous ./db_data to avoid test interference
        if Path::new(crate::db::DB_BASE_PATH).exists() {
            fs::remove_dir_all(crate::db::DB_BASE_PATH).expect("Failed to clean up test DBs before run");
        }

        let dbs = Arc::new(AllMempoolDbs::new().expect("Failed to create test DBs"));
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let config = ConsensusConfig::default();
        let node = ConsensusNode::new(node_id.to_string(), Arc::clone(&dbs), tx, config);

        (node, rx, dbs)
    }

    fn cleanup_test_environment() {
        if Path::new(crate::db::DB_BASE_PATH).exists() {
            fs::remove_dir_all(crate::db::DB_BASE_PATH).expect("Failed to clean up test DBs after run");
        }
    }

    #[tokio::test]
    async fn test_handle_new_transaction_request() {
        let (node, mut rx, _dbs) = setup_test_environment("leader_charlie");

        let mut to_map = HashMap::new();
        to_map.insert("bob_address".to_string(), 1.0);
        let mut from_map = HashMap::new();
        from_map.insert("alice_utxo1".to_string(), 2.0);
        let tx_data = TransactionData {
            to: to_map,
            from: from_map,
            user: "alice_address".to_string(),
            sig: Some("alice_signature".to_string()), // Signature would be validated
            stake: 0.2,
            fee: 0.1,
        };

        let raw_tx_id = node.handle_new_transaction_request(tx_data.clone()).await.unwrap();
        assert!(!raw_tx_id.is_empty() && raw_tx_id != "invalid_tx_data_hash");

        // Check if message was sent on the network
        let sent_message = rx.recv().await.unwrap();
        match sent_message {
            ConsensusMessage::RawTransactionShare { from_node_id, raw_tx_id: rti, raw_tx_entry } => {
                assert_eq!(from_node_id, "leader_charlie");
                assert_eq!(rti, raw_tx_id);
                assert_eq!(raw_tx_entry.tx_data, tx_data);
            }
            _ => panic!("Unexpected message type sent"),
        }

        // Verify DB state (simplified check)
        let charlie_txs_json = _dbs.raw_tx_mempool_db.get(&"leader_charlie".to_string()).unwrap().unwrap();
        let charlie_txs: HashMap<RawTxId, RawTransactionEntry> = serde_json::from_str(&charlie_txs_json).unwrap();
        assert!(charlie_txs.contains_key(&raw_tx_id));
        assert!(_dbs.validation_tasks_mempool_db.get(&raw_tx_id).unwrap().is_some());
        assert!(_dbs.locked_utxo_mempool_db.get(&"alice_utxo1".to_string()).unwrap().is_some());

        cleanup_test_environment();
    }

    #[tokio::test]
    async fn test_process_raw_transaction_share_from_another_leader() {
        let (node_self, _rx_self, dbs_self) = setup_test_environment("leader_delta"); // Our node
        // We need a way to simulate a message arriving *as if* from network
        // So we call process_network_message directly.

        let mut to_map = HashMap::new();
        to_map.insert("bob_address".to_string(), 1.0);
        let mut from_map = HashMap::new();
        from_map.insert("alice_utxo1".to_string(), 2.0);
        let tx_data_orig = TransactionData {
            to: to_map, from: from_map, user: "alice_address".to_string(), sig: Some("alice_sig".to_string()),
            stake: 0.2, fee: 0.1,
        };
        let raw_tx_id_orig = ConsensusNode::calculate_raw_tx_id(&tx_data_orig);
        let raw_tx_entry_orig = RawTransactionEntry {
            tx_data: tx_data_orig.clone(),
            validation_timestamps: vec![],
            validation_tasks: vec![],
            tx_timestamp: Utc::now().timestamp_millis(),
        };

        let share_message = ConsensusMessage::RawTransactionShare {
            from_node_id: "leader_charlie".to_string(), // Message is from Charlie
            raw_tx_id: raw_tx_id_orig.clone(),
            raw_tx_entry: raw_tx_entry_orig.clone(),
        };

        node_self.process_network_message(share_message).await.unwrap();

        // Verify DB state on "leader_delta"
        // Raw tx should be stored under "leader_charlie"'s key in delta's raw_tx_mempool
        let charlie_txs_on_delta_json = dbs_self.raw_tx_mempool_db.get(&"leader_charlie".to_string()).unwrap().unwrap();
        let charlie_txs_on_delta: HashMap<RawTxId, RawTransactionEntry> = serde_json::from_str(&charlie_txs_on_delta_json).unwrap();
        assert!(charlie_txs_on_delta.contains_key(&raw_tx_id_orig));
        assert_eq!(charlie_txs_on_delta.get(&raw_tx_id_orig).unwrap().tx_data, tx_data_orig);

        // Check validation_tasks_mempool and locked_utxo_mempool on delta
        assert!(dbs_self.validation_tasks_mempool_db.get(&raw_tx_id_orig).unwrap().is_some());
        assert!(dbs_self.locked_utxo_mempool_db.get(&"alice_utxo1".to_string()).unwrap().is_some());

        cleanup_test_environment();
    }

    // TODO: Add more tests:
    // - test_validation_task_submission_success_and_processing: DONE
    //   - Setup a raw_tx in node's own mempool.
    //   - Simulate ValidationTaskSubmission.
    //   - Check if it moves to processing_tx_mempool and ProcessingTransactionShare is sent.
    // - test_validation_task_submission_insufficient_validations
    // - test_processing_transaction_share_from_another_leader
    // - test_invalidation_logic (when implemented)
    // - test_leader_election_logic (when implemented)

    #[tokio::test]
    async fn test_validation_task_submission_success_and_processing() {
        let leader_node_id = "leader_charlie_processing_test".to_string();
        let mut config = ConsensusConfig::default();
        config.required_validation_timestamps = 2; // Lower for easier testing

        let (node, mut network_rx, dbs) = setup_test_environment(&leader_node_id);
        node.config.required_validation_timestamps = config.required_validation_timestamps;


        // 1. Setup: Manually inject a RawTransactionEntry into the leader's mempool
        let mut to_map = HashMap::new();
        to_map.insert("bob_address_proc".to_string(), 1.0);
        let mut from_map = HashMap::new();
        from_map.insert("alice_utxo_proc1".to_string(), 2.0);
        let tx_data = TransactionData {
            to: to_map,
            from: from_map,
            user: "alice_address_proc".to_string(),
            sig: Some("alice_proc_signature".to_string()),
            stake: 0.2,
            fee: 0.1,
        };
        let raw_tx_id = ConsensusNode::calculate_raw_tx_id(&tx_data);

        // Define tasks that this leader is supposed to have assigned
        let task1 = ValidationTaskItem { task_id: "task_proc_1".to_string(), complete: false, assigned_by_leader_id: leader_node_id.clone() };
        let task2 = ValidationTaskItem { task_id: "task_proc_2".to_string(), complete: false, assigned_by_leader_id: leader_node_id.clone() };
        // let task3 = ValidationTaskItem { task_id: "task_proc_3".to_string(), complete: false, assigned_by_leader_id: leader_node_id.clone() };


        let raw_tx_entry_original = RawTransactionEntry {
            tx_data: tx_data.clone(),
            validation_timestamps: Vec::new(),
            validation_tasks: vec![task1.clone(), task2.clone()], // Tasks assigned by this leader
            tx_timestamp: Utc::now().timestamp_millis(),
        };

        // Store in leader's own raw_tx_mempool segment
        let mut leader_raw_txs: HashMap<RawTxId, RawTransactionEntry> = HashMap::new();
        leader_raw_txs.insert(raw_tx_id.clone(), raw_tx_entry_original.clone());
        let leader_raw_txs_json = serde_json::to_string(&leader_raw_txs).unwrap();
        dbs.raw_tx_mempool_db.put(&leader_node_id, &leader_raw_txs_json).unwrap();

        // Store in validation_tasks_mempool (as if it's awaiting these tasks)
        let validation_tasks_for_mempool_json = serde_json::to_string(&raw_tx_entry_original.validation_tasks).unwrap();
        dbs.validation_tasks_mempool_db.put(&raw_tx_id, &validation_tasks_for_mempool_json).unwrap();

        // 2. Simulate ValidationTaskSubmissions
        let submission1 = ConsensusMessage::ValidationTaskSubmission {
            from_user_or_validator_id: "validator_alpha".to_string(),
            raw_tx_id: raw_tx_id.clone(),
            completed_tasks: vec![ValidationTaskItem { task_id: task1.task_id.clone(), complete: true, assigned_by_leader_id: leader_node_id.clone() }],
        };
        node.process_network_message(submission1).await.unwrap();

        // Check: Not enough validations yet, should not have sent ProcessingTransactionShare
        assert!(network_rx.try_recv().is_err(), "Should not send ProcessingTxShare with 1 validation yet");
        let current_leader_raw_txs_json = dbs.raw_tx_mempool_db.get(&leader_node_id).unwrap().unwrap();
        let current_leader_raw_txs: HashMap<RawTxId, RawTransactionEntry> = serde_json::from_str(&current_leader_raw_txs_json).unwrap();
        assert!(current_leader_raw_txs.contains_key(&raw_tx_id), "Raw TX should still be in mempool after 1 validation");
        assert_eq!(current_leader_raw_txs.get(&raw_tx_id).unwrap().validation_timestamps.len(), 1, "Should have 1 timestamp");


        let submission2 = ConsensusMessage::ValidationTaskSubmission {
            from_user_or_validator_id: "validator_beta".to_string(),
            raw_tx_id: raw_tx_id.clone(),
            completed_tasks: vec![ValidationTaskItem { task_id: task2.task_id.clone(), complete: true, assigned_by_leader_id: leader_node_id.clone() }],
        };
        node.process_network_message(submission2).await.unwrap();

        // 3. Assertions after sufficient validations
        // Check network message for ProcessingTransactionShare
        let sent_message = network_rx.recv().await.unwrap();
        let final_tx_id_check: TxId;
        match sent_message {
            ConsensusMessage::ProcessingTransactionShare { from_node_id: sent_from_node_id, tx_id, processing_tx_entry } => {
                assert_eq!(sent_from_node_id, leader_node_id);
                assert!(!tx_id.is_empty());
                final_tx_id_check = tx_id.clone(); // Store for DB check
                assert_eq!(processing_tx_entry.leader_id, leader_node_id);
                assert_eq!(processing_tx_entry.tx_data_with_avg_ts.values().next().unwrap().user, tx_data.user);
                // TODO: check signature on processing_tx_entry if we implement real signing
            }
            _ => panic!("Unexpected message type sent. Expected ProcessingTransactionShare. Got: {:?}", sent_message),
        }

        // Check DB state:
        // Raw tx should be removed from leader's raw_tx_mempool
        let final_leader_raw_txs_json = dbs.raw_tx_mempool_db.get(&leader_node_id).unwrap().unwrap();
        let final_leader_raw_txs: HashMap<RawTxId, RawTransactionEntry> = serde_json::from_str(&final_leader_raw_txs_json).unwrap();
        assert!(!final_leader_raw_txs.contains_key(&raw_tx_id), "Raw TX should be removed from mempool after processing");

        // Raw tx should be removed from validation_tasks_mempool
        assert!(dbs.validation_tasks_mempool_db.get(&raw_tx_id).unwrap().is_none(), "Entry should be removed from validation_tasks_mempool");

        // Processing tx should be in processing_tx_mempool
        let processing_entry_json = dbs.processing_tx_mempool_db.get(&final_tx_id_check).unwrap().unwrap();
        let processing_entry_db: ProcessingTransactionEntry = serde_json::from_str(&processing_entry_json).unwrap();
        assert_eq!(processing_entry_db.leader_id, leader_node_id);
        assert_eq!(processing_entry_db.tx_data_with_avg_ts.values().next().unwrap().user, tx_data.user);

        cleanup_test_environment();
    }

    #[tokio::test]
    async fn test_process_processing_transaction_share() {
        let self_node_id = "leader_delta_proc_share_test".to_string();
        let other_leader_node_id = "leader_charlie_originator".to_string();

        let (node_delta, mut network_rx, dbs_delta) = setup_test_environment(&self_node_id);

        // 1. Create a ProcessingTransactionEntry (as if from Charlie)
        let mut to_map = HashMap::new();
        to_map.insert("bob_final_dest".to_string(), 5.0);
        let mut from_map = HashMap::new();
        from_map.insert("alice_final_utxo".to_string(), 6.0);
        let tx_data = TransactionData {
            to: to_map, from: from_map, user: "alice_final_user".to_string(),
            sig: Some("alice_final_sig".to_string()), stake: 0.5, fee: 0.05,
        };

        let avg_timestamp = Utc::now().timestamp_millis() - 10000; // Some past time
        let final_tx_id = ConsensusNode::calculate_final_tx_id(avg_timestamp, &tx_data);

        let mut tx_data_with_avg_ts = HashMap::new();
        tx_data_with_avg_ts.insert(avg_timestamp, tx_data.clone());

        let processing_tx_entry_from_charlie = ProcessingTransactionEntry {
            tx_data_with_avg_ts,
            sig: format!("charlie_sig_on_{}", final_tx_id),
            leader_id: other_leader_node_id.clone(),
        };

        // 2. Create the network message
        let message = ConsensusMessage::ProcessingTransactionShare {
            from_node_id: other_leader_node_id.clone(),
            tx_id: final_tx_id.clone(),
            processing_tx_entry: processing_tx_entry_from_charlie.clone(),
        };

        // 3. Process the message
        node_delta.process_network_message(message).await.unwrap();

        // 4. Assertions
        // Check DB state on Delta: processing_tx_mempool_db should contain the entry
        let entry_in_delta_db_json = dbs_delta.processing_tx_mempool_db.get(&final_tx_id).unwrap()
            .expect("Processing TX entry should be in Delta's DB");
        let entry_in_delta_db: ProcessingTransactionEntry = serde_json::from_str(&entry_in_delta_db_json).unwrap();

        assert_eq!(entry_in_delta_db.leader_id, other_leader_node_id, "Leader ID mismatch in stored entry");
        assert_eq!(entry_in_delta_db.tx_data_with_avg_ts.values().next().unwrap().user, tx_data.user, "User data mismatch");
        assert_eq!(entry_in_delta_db.sig, processing_tx_entry_from_charlie.sig, "Signature mismatch");

        // Assert that Delta node does not send out any new messages in response to this (as per current simple logic)
        assert!(network_rx.try_recv().is_err(), "Delta node should not gossip further on receiving ProcessingTransactionShare in this basic test");

        // TODO: Future tests might check for removal of corresponding raw_tx_id if that logic is added.
        // TODO: Future tests might check for initiation of finality validation tasks.

        cleanup_test_environment();
    }

    #[tokio::test]
    async fn test_validation_task_submission_insufficient_validations() {
        let leader_node_id = "leader_charlie_insufficient_test".to_string();
        let mut config = ConsensusConfig::default();
        config.required_validation_timestamps = 2; // Needs 2 validations

        let (node, mut network_rx, dbs) = setup_test_environment(&leader_node_id);
        node.config.required_validation_timestamps = config.required_validation_timestamps;

        // 1. Setup: Manually inject a RawTransactionEntry
        let mut to_map = HashMap::new();
        to_map.insert("bob_insufficient".to_string(), 1.0);
        let mut from_map = HashMap::new();
        from_map.insert("alice_utxo_insufficient".to_string(), 2.0);
        let tx_data = TransactionData {
            to: to_map, from: from_map, user: "alice_insufficient".to_string(),
            sig: Some("alice_insufficient_sig".to_string()), stake: 0.2, fee: 0.1,
        };
        let raw_tx_id = ConsensusNode::calculate_raw_tx_id(&tx_data);

        let task1 = ValidationTaskItem { task_id: "task_insufficient_1".to_string(), complete: false, assigned_by_leader_id: leader_node_id.clone() };
        let task2 = ValidationTaskItem { task_id: "task_insufficient_2".to_string(), complete: false, assigned_by_leader_id: leader_node_id.clone() };

        let raw_tx_entry_original = RawTransactionEntry {
            tx_data: tx_data.clone(),
            validation_timestamps: Vec::new(),
            validation_tasks: vec![task1.clone(), task2.clone()],
            tx_timestamp: Utc::now().timestamp_millis(),
        };

        let mut leader_raw_txs: HashMap<RawTxId, RawTransactionEntry> = HashMap::new();
        leader_raw_txs.insert(raw_tx_id.clone(), raw_tx_entry_original.clone());
        let leader_raw_txs_json = serde_json::to_string(&leader_raw_txs).unwrap();
        dbs.raw_tx_mempool_db.put(&leader_node_id, &leader_raw_txs_json).unwrap();
        let validation_tasks_for_mempool_json = serde_json::to_string(&raw_tx_entry_original.validation_tasks).unwrap();
        dbs.validation_tasks_mempool_db.put(&raw_tx_id, &validation_tasks_for_mempool_json).unwrap();

        // 2. Simulate only ONE ValidationTaskSubmission (when 2 are required)
        let submission1 = ConsensusMessage::ValidationTaskSubmission {
            from_user_or_validator_id: "validator_gamma".to_string(),
            raw_tx_id: raw_tx_id.clone(),
            completed_tasks: vec![ValidationTaskItem { task_id: task1.task_id.clone(), complete: true, assigned_by_leader_id: leader_node_id.clone() }],
        };
        node.process_network_message(submission1).await.unwrap();

        // 3. Assertions
        // No ProcessingTransactionShare message should be sent
        assert!(network_rx.try_recv().is_err(), "Should NOT send ProcessingTxShare with insufficient validations");

        // Raw TX should still be in the leader's raw_tx_mempool
        let current_leader_raw_txs_json = dbs.raw_tx_mempool_db.get(&leader_node_id).unwrap().unwrap();
        let current_leader_raw_txs: HashMap<RawTxId, RawTransactionEntry> = serde_json::from_str(&current_leader_raw_txs_json).unwrap();
        assert!(current_leader_raw_txs.contains_key(&raw_tx_id), "Raw TX should still be in mempool");

        // Timestamp count should be 1
        let entry_in_mempool = current_leader_raw_txs.get(&raw_tx_id).unwrap();
        assert_eq!(entry_in_mempool.validation_timestamps.len(), 1, "Should have 1 validation timestamp");

        // Task1 should be marked complete, Task2 should not
        assert!(entry_in_mempool.validation_tasks.iter().find(|t| t.task_id == task1.task_id).unwrap().complete, "Task1 should be complete");
        assert!(!entry_in_mempool.validation_tasks.iter().find(|t| t.task_id == task2.task_id).unwrap().complete, "Task2 should NOT be complete");

        // processing_tx_mempool should NOT contain this transaction
        assert!(dbs.processing_tx_mempool_db.get(&ConsensusNode::calculate_final_tx_id(0, &tx_data)).unwrap().is_none(),
            "Processing mempool should not contain the tx yet"); // Using a dummy timestamp for check, as it wouldn't be processed

        cleanup_test_environment();
    }
}
