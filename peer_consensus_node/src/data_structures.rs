use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// From README:
// tx_data = {
//     to: [bob_address: 1],
//     from: [alice_utxo1: 2],
//     user: alice_address,
//     sig: alice_signature, // signs this message, without the "sig" property
//     stake: 0.2,
//     fee: 0.1
// } // total of 1.3 coins required, .9 returns to alice_address as new utxo on finality.

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TransactionData {
    pub to: HashMap<String, f64>, // address: amount
    pub from: HashMap<String, f64>, // utxo_id: amount
    pub user: String, // user_address
    pub sig: Option<String>, // Signature of the transaction data (all fields except sig itself)
    pub stake: f64,
    pub fee: f64,
}

// raw_tx_mempool = {
//     charlie_id: {
//         raw_tx_id: {
//             tx_data,
//             validation_timestamps: [],
//             validation_tasks: [],
//             tx_timestamp: 1751728707356
//         }
//     }
// }
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RawTransactionEntry {
    pub tx_data: TransactionData,
    pub validation_timestamps: Vec<i64>,
    pub validation_tasks: Vec<ValidationTaskItem>, // Changed from generic Vec<String> to Vec<ValidationTaskItem>
    pub tx_timestamp: i64,
}

// validation_tasks = [
//     leader2_id: [
//         {task: task_id1, complete: false}, {task_id2, complete: false}
//     ],
//     leader8_id: [
//         {task: task_id1, complete: false}, {task: task_id2, complete: false}
//     ]
// ]
// This structure seems to be part of RawTransactionEntry, let's define the inner part.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ValidationTaskItem {
    pub task_id: String,
    pub complete: bool,
    // Added leader_id to track who assigned the task, useful for reporting back.
    pub assigned_by_leader_id: String,
}


// validation_tasks_mempool: This seems to be a list of raw_tx_ids that need validation.
// The README says: "Charlie also adds Alice's raw_tx_id to the validation_tasks_mempool."
// So, it could be HashMap<RawTxId, bool> where bool indicates if it's actively being processed or pending.
// Or, more simply, a Set<RawTxId> if we only care about existence.
// Let's assume for now it's a list of raw_tx_ids that require validation tasks.
// We can refine this if needed. The README also says "The number of tasks is proportionate to total tasks in validation_tasks_mempool divided by validators available."
// This implies `validation_tasks_mempool` might store the tasks themselves, or pointers to them.
// Let's reconsider: "Charlie approves validation tasks to send to Alice sorting by the tx_timestamp on other raw transactions."
// "The other leaders send Charlie validation tasks for Alice to complete."
// This suggests `validation_tasks_mempool` might be more like:
// HashMap<RawTxId, HashMap<LeaderId, Vec<ValidationTaskDefinition>>>
// where ValidationTaskDefinition is what Alice needs to do.
// For now, let's keep it simple as a list of RawTxIds that need validation.
// The actual tasks are stored within RawTransactionEntry.

// locked_utxo_mempool: utxos in this list are invalidated from entry into raw_tx_mempool entries
// This could be a HashMap<UtxoId, Timestamp> where Timestamp is when it was locked.
// Or a Set<UtxoId>. Let's use a Set for simplicity.

// processing_tx_mempool: {
//     tx_id: { // timestamp used is the average generated in step 5
//         1751730407001: tx_data,
//         sig: charlie_signature
//         leader: charlie_id
//     }
// }
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProcessingTransactionEntry {
    pub tx_data_with_avg_ts: HashMap<i64, TransactionData>, // avg_validation_timestamp: tx_data
    pub sig: String,      // Leader's signature
    pub leader_id: String, // Leader's Node ID
}

// tx_mempool: transactions approved to be blocked or otherwise finalized in a DLT's protocol
// This would likely be similar to processing_tx_mempool but for finalized transactions.
// The structure might depend on the specific DLT's requirements (e.g., XMBL's Cubic DLT).
// For now, let's assume it stores the tx_id and perhaps some metadata about its finality.
// Example: HashMap<TxId, FinalityProof>
// For XMBL: "calculate the digital root of the tx_id and put into a tx_mempool"
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FinalizedTransactionEntry {
    pub tx_id: String, // The hash of {timestamp: tx_data}
    pub digital_root: u32, // For XMBL's Cubic DLT example
    // Might include other DLT specific finality data
}


// Uptime Mempool Entry
// {
//     45.228.345: { // Node IP (or Node ID)
//         1751739100498: [1, 201] // timestamp_of_last_pulse: [count, average_response_time_ms]
//     }
// }
// We'll use NodeId (String) as key instead of IP for flexibility.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UptimeEntry {
    pub last_pulse_timestamp: i64,
    pub pulse_count: u64,
    pub average_response_time_ms: u64,
}

// Type aliases for better readability
pub type RawTxId = String; // Hash of raw transaction data
pub type TxId = String; // Hash of {avg_timestamp: tx_data}
pub type NodeId = String; // Unique identifier for a node
pub type UtxoId = String; // Unique identifier for a UTXO

// Mempool state (simplified, will be backed by RocksDB)
// These are conceptual representations. The actual storage will use RocksDB instances.
pub struct Mempools {
    pub raw_tx_mempool: HashMap<NodeId, HashMap<RawTxId, RawTransactionEntry>>,
    pub validation_tasks_mempool: HashMap<RawTxId, Vec<ValidationTaskItem>>, // Stores tasks that need to be completed for a raw_tx_id
    pub locked_utxo_mempool: HashMap<UtxoId, i64>, // UtxoId -> lock_timestamp
    pub processing_tx_mempool: HashMap<TxId, ProcessingTransactionEntry>,
    pub tx_mempool: HashMap<TxId, FinalizedTransactionEntry>, // For finalized TXs per DLT requirements
    pub uptime_mempool: HashMap<NodeId, UptimeEntry>,
}

impl Mempools {
    pub fn new() -> Self {
        Mempools {
            raw_tx_mempool: HashMap::new(),
            validation_tasks_mempool: HashMap::new(),
            locked_utxo_mempool: HashMap::new(),
            processing_tx_mempool: HashMap::new(),
            tx_mempool: HashMap::new(),
            uptime_mempool: HashMap::new(),
        }
    }
}

// For Node Identity
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct NodeIdentity {
    pub ip_address: String, // Assuming IPv4 or IPv6 string
    pub user_public_key: String, // Public key of the user running the node
    pub signature: String, // Signature of ip_address by user_private_key
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transaction_data_serialization() {
        let mut to_map = HashMap::new();
        to_map.insert("bob_address".to_string(), 1.0);
        let mut from_map = HashMap::new();
        from_map.insert("alice_utxo1".to_string(), 2.0);

        let tx = TransactionData {
            to: to_map,
            from: from_map,
            user: "alice_address".to_string(),
            sig: Some("alice_signature".to_string()),
            stake: 0.2,
            fee: 0.1,
        };
        let serialized = serde_json::to_string(&tx).unwrap();
        let deserialized: TransactionData = serde_json::from_str(&serialized).unwrap();
        assert_eq!(tx, deserialized);
    }
}
