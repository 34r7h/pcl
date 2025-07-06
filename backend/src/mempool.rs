// Mempool module - TODO: Implement mempool functionality 

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use crate::transaction::{RawTransaction, ValidationTask, ProcessingTransaction, TransactionData};
use crate::error::{PclError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawTxMempool {
    pub transactions: HashMap<String, RawTransaction>,
    pub hash_to_tx: HashMap<String, String>, // hash -> tx_id
    pub tx_by_user: HashMap<String, Vec<String>>, // user -> tx_ids
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationTasksMempool {
    pub tasks: HashMap<String, ValidationTask>,
    pub assigned_tasks: HashMap<String, Vec<String>>, // leader_id -> task_ids
    pub user_tasks: HashMap<String, Vec<String>>, // user_id -> task_ids
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockedUtxoMempool {
    pub locked_utxos: HashMap<String, LockedUtxo>, // utxo_id -> locked_utxo
    pub tx_locks: HashMap<String, Vec<String>>, // tx_id -> utxo_ids
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingTxMempool {
    pub transactions: HashMap<String, ProcessingTransaction>,
    pub timestamp_averages: HashMap<String, DateTime<Utc>>, // tx_id -> average_timestamp
    pub signatures: HashMap<String, String>, // tx_id -> leader_signature
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxMempool {
    pub finalized_transactions: HashMap<String, FinalizedTransaction>,
    pub xmbl_integrated: HashMap<String, XmblIntegration>, // tx_id -> xmbl_data
    pub utxo_pool: HashMap<String, UtxoEntry>, // utxo_id -> utxo
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UptimeMempool {
    pub pulse_data: HashMap<String, PulseData>, // node_id -> pulse_data
    pub family_responses: HashMap<Uuid, Vec<PulseResponse>>, // family_id -> responses
    pub response_times: HashMap<String, Vec<u64>>, // node_id -> response_times_ms
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockedUtxo {
    pub utxo_id: String,
    pub amount: f64,
    pub locked_by_tx: String,
    pub locked_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinalizedTransaction {
    pub tx_id: String,
    pub tx_data: TransactionData,
    pub xmbl_cubic_root: u8,
    pub validator_signature: String,
    pub finalized_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XmblIntegration {
    pub tx_id: String,
    pub digital_root: u8,
    pub cubic_position: u64,
    pub integrated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtxoEntry {
    pub utxo_id: String,
    pub amount: f64,
    pub owner: String,
    pub created_at: DateTime<Utc>,
    pub spent: bool,
}

// Data stored in UptimeMempool about each *other* node that this node observes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservedNodePulseData {
    pub node_id: String, // PeerId of the observed node
    pub last_pulse_received_at: Option<DateTime<Utc>>,
    // History of actual pulse timestamps received from this node. Prune periodically.
    pub pulse_receipt_history: Vec<DateTime<Utc>>,
    // History of response times received *from* this node for pulses *we* sent. Prune periodically.
    pub response_time_samples_ms: Vec<u64>,
    // Could add first_seen, etc. for more robust uptime calculation over defined windows.
}

impl ObservedNodePulseData {
    fn new(node_id: String) -> Self {
        Self {
            node_id,
            last_pulse_received_at: None,
            pulse_receipt_history: Vec::new(),
            response_time_samples_ms: Vec::new(),
        }
    }

    fn record_pulse_receipt(&mut self, received_at: DateTime<Utc>) {
        self.last_pulse_received_at = Some(received_at);
        self.pulse_receipt_history.push(received_at);
        // Prune history (e.g., keep last N or within X duration)
        if self.pulse_receipt_history.len() > 100 { // Example: keep last 100
            self.pulse_receipt_history.drain(0..self.pulse_receipt_history.len() - 100);
        }
    }

    fn record_response_time(&mut self, rt_ms: u64) {
        self.response_time_samples_ms.push(rt_ms);
        // Prune history
        if self.response_time_samples_ms.len() > 50 { // Example: keep last 50
            self.response_time_samples_ms.drain(0..self.response_time_samples_ms.len() - 50);
        }
    }

    // Calculates uptime based on recent pulse history.
    // `pulse_interval_secs`: The expected interval of pulses from other nodes.
    // `window_duration_secs`: How far back to look for uptime calculation.
    // Returns a percentage (0.0 to 100.0).
    fn calculate_uptime(&self, pulse_interval_secs: u64, window_duration_secs: u64) -> f64 {
        if self.pulse_receipt_history.is_empty() {
            return 0.0;
        }
        let window_start_time = Utc::now() - chrono::Duration::seconds(window_duration_secs as i64);

        let pulses_in_window = self.pulse_receipt_history.iter()
            .filter(|&&t| t >= window_start_time)
            .count();

        // Max number of pulses expected in the window.
        // Add 1 to account for the pulse at the very start of the window.
        let max_expected_pulses_in_window = (window_duration_secs / pulse_interval_secs).max(1); // Avoid division by zero if interval is 0

        if max_expected_pulses_in_window == 0 { return 100.0; } // Should not happen with max(1)

        (pulses_in_window as f64 / max_expected_pulses_in_window as f64).min(1.0) * 100.0
    }

    fn get_average_response_time(&self) -> Option<f64> {
        if self.response_time_samples_ms.is_empty() {
            return None;
        }
        let sum: u64 = self.response_time_samples_ms.iter().sum();
        Some(sum as f64 / self.response_time_samples_ms.len() as f64)
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PulseResponse {
    pub responder_id: String,
    pub pulse_id: String,
    pub response_time_ms: u64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MempoolManager {
    pub raw_tx: RawTxMempool,
    pub validation_tasks: ValidationTasksMempool,
    pub locked_utxo: LockedUtxoMempool,
    pub processing_tx: ProcessingTxMempool,
    pub tx: TxMempool,
    pub uptime: UptimeMempool,
}

impl Default for MempoolManager {
    fn default() -> Self {
        Self::new()
    }
}

impl MempoolManager {
    pub fn new() -> Self {
        Self {
            raw_tx: RawTxMempool::new(),
            validation_tasks: ValidationTasksMempool::new(),
            locked_utxo: LockedUtxoMempool::new(),
            processing_tx: ProcessingTxMempool::new(),
            tx: TxMempool::new(),
            uptime: UptimeMempool::new(),
        }
    }

    pub fn add_raw_transaction(&mut self, tx: RawTransaction) -> Result<()> {
        self.raw_tx.add_transaction(tx)
    }

    pub fn remove_raw_transaction(&mut self, tx_id: &str) -> Result<()> {
        self.raw_tx.remove_transaction(tx_id)
    }

    pub fn add_validation_task(&mut self, task: ValidationTask) -> Result<()> {
        self.validation_tasks.add_task(task)
    }

    pub fn lock_utxo(&mut self, utxo_id: String, amount: f64, tx_id: String) -> Result<()> {
        self.locked_utxo.lock_utxo(utxo_id, amount, tx_id)
    }

    pub fn unlock_utxo(&mut self, utxo_id: &str) -> Result<()> {
        self.locked_utxo.unlock_utxo(utxo_id)
    }

    pub fn add_processing_transaction(&mut self, tx: ProcessingTransaction) -> Result<()> {
        self.processing_tx.add_transaction(tx)
    }

    pub fn finalize_transaction(&mut self, tx_id: String, validator_sig: String) -> Result<()> {
        self.tx.finalize_transaction(tx_id, validator_sig)
    }

    // Call this when this node *receives* a PulseMessage from another node
    pub fn record_received_pulse(&mut self, sender_peer_id: String, family_id_pulsed: Uuid, pulse_received_at: DateTime<Utc>) -> Result<()> {
        self.uptime.record_received_pulse(sender_peer_id, family_id_pulsed, pulse_received_at)
    }

    // Call this when this node *receives* a PulseResponseMessage from another node
    pub fn record_received_pulse_response(&mut self, responder_peer_id: String, original_pulse_id: String, response_time_ms: u64, response_received_at: DateTime<Utc>) -> Result<()> {
        self.uptime.record_received_pulse_response(responder_peer_id, original_pulse_id, response_time_ms, response_received_at)
    }

    pub fn calculate_node_uptime_percentage(&self, node_peer_id: &str) -> f64 {
        self.uptime.calculate_uptime_percentage(node_peer_id)
    }

    pub fn get_node_average_response_time(&self, node_peer_id: &str) -> Option<f64> {
        self.uptime.get_average_response_time(node_peer_id)
    }

    pub fn invalidate_transaction(&mut self, tx_id: &str) -> Result<()> {
        // Remove from all mempools
        let _ = self.raw_tx.remove_transaction(tx_id);
        let _ = self.processing_tx.remove_transaction(tx_id);
        let _ = self.validation_tasks.remove_tasks_for_tx(tx_id);
        let _ = self.locked_utxo.unlock_utxos_for_tx(tx_id);
        Ok(())
    }

    pub fn get_mempool_stats(&self) -> MempoolStats {
        MempoolStats {
            raw_tx_count: self.raw_tx.transactions.len(),
            validation_tasks_count: self.validation_tasks.tasks.len(),
            locked_utxo_count: self.locked_utxo.locked_utxos.len(),
            processing_tx_count: self.processing_tx.transactions.len(),
            finalized_tx_count: self.tx.finalized_transactions.len(),
            active_nodes: self.uptime.pulse_data.len(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MempoolStats {
    pub raw_tx_count: usize,
    pub validation_tasks_count: usize,
    pub locked_utxo_count: usize,
    pub processing_tx_count: usize,
    pub finalized_tx_count: usize,
    pub active_nodes: usize,
}

impl RawTxMempool {
    pub fn new() -> Self {
        Self {
            transactions: HashMap::new(),
            hash_to_tx: HashMap::new(),
            tx_by_user: HashMap::new(),
        }
    }

    pub fn add_transaction(&mut self, tx: RawTransaction) -> Result<()> {
        let tx_id = tx.raw_tx_id.clone();
        let user = tx.tx_data.user.clone();
        
        // Calculate transaction hash
        let hash = crate::crypto::hash_transaction_data(&serde_json::to_vec(&tx.tx_data)?);
        let hash_str = hex::encode(hash);
        
        self.hash_to_tx.insert(hash_str, tx_id.clone());
        self.tx_by_user.entry(user).or_insert_with(Vec::new).push(tx_id.clone());
        self.transactions.insert(tx_id, tx);
        
        Ok(())
    }

    pub fn remove_transaction(&mut self, tx_id: &str) -> Result<()> {
        if let Some(tx) = self.transactions.remove(tx_id) {
            // Remove from hash map
            let hash = crate::crypto::hash_transaction_data(&serde_json::to_vec(&tx.tx_data)?);
            let hash_str = hex::encode(hash);
            self.hash_to_tx.remove(&hash_str);
            
            // Remove from user transactions
            if let Some(user_txs) = self.tx_by_user.get_mut(&tx.tx_data.user) {
                user_txs.retain(|id| id != tx_id);
                if user_txs.is_empty() {
                    self.tx_by_user.remove(&tx.tx_data.user);
                }
            }
        }
        Ok(())
    }

    pub fn get_transaction(&self, tx_id: &str) -> Option<&RawTransaction> {
        self.transactions.get(tx_id)
    }

    pub fn get_transaction_by_hash(&self, hash: &str) -> Option<&RawTransaction> {
        self.hash_to_tx.get(hash)
            .and_then(|tx_id| self.transactions.get(tx_id))
    }
}

impl ValidationTasksMempool {
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
            assigned_tasks: HashMap::new(),
            user_tasks: HashMap::new(),
        }
    }

    pub fn add_task(&mut self, task: ValidationTask) -> Result<()> {
        let task_id = task.task_id.clone();
        let leader_id = task.leader_id.clone();
        
        self.assigned_tasks.entry(leader_id).or_insert_with(Vec::new).push(task_id.clone());
        self.tasks.insert(task_id, task);
        
        Ok(())
    }

    pub fn complete_task(&mut self, task_id: &str) -> Result<()> {
        if let Some(task) = self.tasks.get_mut(task_id) {
            task.complete();
        }
        Ok(())
    }

    pub fn remove_tasks_for_tx(&mut self, tx_id: &str) -> Result<()> {
        let task_ids: Vec<String> = self.tasks.keys().cloned().collect();
        for task_id in task_ids {
            if task_id.starts_with(tx_id) {
                self.tasks.remove(&task_id);
            }
        }
        Ok(())
    }
}

impl LockedUtxoMempool {
    pub fn new() -> Self {
        Self {
            locked_utxos: HashMap::new(),
            tx_locks: HashMap::new(),
        }
    }

    pub fn lock_utxo(&mut self, utxo_id: String, amount: f64, tx_id: String) -> Result<()> {
        let locked_utxo = LockedUtxo {
            utxo_id: utxo_id.clone(),
            amount,
            locked_by_tx: tx_id.clone(),
            locked_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::minutes(30), // 30 minute lock
        };
        
        self.locked_utxos.insert(utxo_id.clone(), locked_utxo);
        self.tx_locks.entry(tx_id).or_insert_with(Vec::new).push(utxo_id);
        
        Ok(())
    }

    pub fn unlock_utxo(&mut self, utxo_id: &str) -> Result<()> {
        if let Some(locked_utxo) = self.locked_utxos.remove(utxo_id) {
            if let Some(tx_locks) = self.tx_locks.get_mut(&locked_utxo.locked_by_tx) {
                tx_locks.retain(|id| id != utxo_id);
                if tx_locks.is_empty() {
                    self.tx_locks.remove(&locked_utxo.locked_by_tx);
                }
            }
        }
        Ok(())
    }

    pub fn unlock_utxos_for_tx(&mut self, tx_id: &str) -> Result<()> {
        if let Some(utxo_ids) = self.tx_locks.remove(tx_id) {
            for utxo_id in utxo_ids {
                self.locked_utxos.remove(&utxo_id);
            }
        }
        Ok(())
    }

    pub fn is_utxo_locked(&self, utxo_id: &str) -> bool {
        self.locked_utxos.contains_key(utxo_id)
    }
}

impl ProcessingTxMempool {
    pub fn new() -> Self {
        Self {
            transactions: HashMap::new(),
            timestamp_averages: HashMap::new(),
            signatures: HashMap::new(),
        }
    }

    pub fn add_transaction(&mut self, tx: ProcessingTransaction) -> Result<()> {
        let tx_id = tx.tx_id.clone();
        let signature = tx.sig.clone();
        let timestamp = tx.timestamp;
        
        self.timestamp_averages.insert(tx_id.clone(), timestamp);
        self.signatures.insert(tx_id.clone(), signature);
        self.transactions.insert(tx_id, tx);
        
        Ok(())
    }

    pub fn remove_transaction(&mut self, tx_id: &str) -> Result<()> {
        self.transactions.remove(tx_id);
        self.timestamp_averages.remove(tx_id);
        self.signatures.remove(tx_id);
        Ok(())
    }
}

impl TxMempool {
    pub fn new() -> Self {
        Self {
            finalized_transactions: HashMap::new(),
            xmbl_integrated: HashMap::new(),
            utxo_pool: HashMap::new(),
        }
    }

    pub fn finalize_transaction(&mut self, tx_id: String, validator_sig: String) -> Result<()> {
        // This would normally get the transaction from processing mempool
        // For now, create a placeholder
        let tx_data = TransactionData::new(
            vec![("placeholder".to_string(), 1.0)],
            vec![("placeholder".to_string(), 1.0)],
            "placeholder".to_string(),
            0.1,
            0.01,
        );
        
        let finalized_tx = FinalizedTransaction {
            tx_id: tx_id.clone(),
            tx_data: tx_data.clone(),
            xmbl_cubic_root: tx_data.calculate_digital_root() as u8,
            validator_signature: validator_sig,
            finalized_at: Utc::now(),
        };
        
        self.finalized_transactions.insert(tx_id, finalized_tx);
        Ok(())
    }

    pub fn integrate_xmbl(&mut self, tx_id: String, digital_root: u8, cubic_position: u64) -> Result<()> {
        let integration = XmblIntegration {
            tx_id: tx_id.clone(),
            digital_root,
            cubic_position,
            integrated_at: Utc::now(),
        };
        
        self.xmbl_integrated.insert(tx_id, integration);
        Ok(())
    }

    pub fn create_utxo(&mut self, utxo_id: String, amount: f64, owner: String) -> Result<()> {
        let utxo = UtxoEntry {
            utxo_id: utxo_id.clone(),
            amount,
            owner,
            created_at: Utc::now(),
            spent: false,
        };
        
        self.utxo_pool.insert(utxo_id, utxo);
        Ok(())
    }
}

// UptimeMempool stores ObservedNodePulseData for other nodes.
// It also needs to know the expected pulse interval and window for uptime calculations.
// These could be configurable.
const DEFAULT_EXPECTED_PULSE_INTERVAL_SECS: u64 = 20; // From README
const DEFAULT_UPTIME_WINDOW_SECS: u64 = 300; // e.g., 5 minutes, should be multiple of interval

impl UptimeMempool {
    pub fn new() -> Self {
        Self {
            // node_id (PeerId string) -> ObservedNodePulseData
            pulse_data: HashMap::new(),
            // family_id -> Vec<PulseResponse (from nodes in that family responding to our pulse)>
            // This might be less useful if pulses are family-to-family, not specific node to family.
            // For now, let's assume it stores responses to pulses *we* sent.
            family_responses: HashMap::new(),
             // This seems redundant if ObservedNodePulseData stores response_time_samples_ms
            response_times: HashMap::new(), // node_id (PeerId str) -> Vec<response_times_ms>
        }
    }

    // Called when this node *receives* a PulseMessage from another node (identified by its Node UUID)
    pub fn record_received_pulse(&mut self, sender_node_uuid: String, _family_id_pulsed: Uuid, pulse_received_at: DateTime<Utc>) -> Result<()> {
        let data = self.pulse_data.entry(sender_node_uuid.clone()).or_insert_with(|| ObservedNodePulseData::new(sender_node_uuid));
        data.record_pulse_receipt(pulse_received_at);
        Ok(())
    }

    // Called when this node *receives* a PulseResponseMessage from another node (identified by its Node UUID)
    // (in response to a pulse *we* sent earlier, identified by original_pulse_id)
    pub fn record_received_pulse_response(&mut self, responder_node_uuid: String, _original_pulse_id: String, response_time_ms: u64, _response_received_at: DateTime<Utc>) -> Result<()> {
        let data = self.pulse_data.entry(responder_node_uuid.clone()).or_insert_with(|| ObservedNodePulseData::new(responder_node_uuid));
        data.record_response_time(response_time_ms);

        // The separate self.response_times map is largely redundant now if ObservedNodePulseData handles it.
        // For consistency, if we keep it, it should also be keyed by node_uuid.
        // Let's remove its direct update here and rely on ObservedNodePulseData for avg response time.
        // self.response_times.entry(data.node_id.clone()).or_insert_with(Vec::new).push(response_time_ms);
        // if self.response_times.get(&data.node_id).map_or(false, |v| v.len() > 50) {
        //      self.response_times.get_mut(&data.node_id).unwrap().drain(0..1);
        // }
        Ok(())
    }

    // Calculates uptime for a given node (identified by its Node UUID) based on its observed pulse history.
    pub fn calculate_uptime_percentage(&self, node_uuid: &str) -> f64 {
        if let Some(data) = self.pulse_data.get(node_uuid) {
            data.calculate_uptime(DEFAULT_EXPECTED_PULSE_INTERVAL_SECS, DEFAULT_UPTIME_WINDOW_SECS)
        } else {
            0.0 // Node not observed or no pulses recorded
        }
    }

    // Gets average response time for a given node (identified by its Node UUID) based on its recorded responses.
    pub fn get_average_response_time(&self, node_uuid: &str) -> Option<f64> {
        self.pulse_data.get(node_uuid)
            .and_then(|data| data.get_average_response_time())
    }

    // Method to remove old/inactive nodes from pulse_data to prevent unbounded growth
    pub fn prune_inactive_nodes(&mut self, inactivity_threshold_secs: i64) {
        let threshold_time = Utc::now() - chrono::Duration::seconds(inactivity_threshold_secs);
        self.pulse_data.retain(|_node_uuid, data| { // Key is now node_uuid
            data.last_pulse_received_at.map_or(false, |last_seen| last_seen >= threshold_time) ||
            (!data.response_time_samples_ms.is_empty())
        });
        // Also prune the redundant self.response_times if it's kept
        self.response_times.retain(|node_uuid, times| {
            self.pulse_data.contains_key(node_uuid) && !times.is_empty()
        });
    }
}