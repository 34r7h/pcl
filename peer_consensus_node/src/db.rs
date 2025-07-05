use rocksdb::{DB, Options, Error as RocksDbError};
use serde::{Serialize, de::DeserializeOwned};
use std::marker::PhantomData;
use std::path::Path;

const DB_BASE_PATH: &str = "./db_data/"; // Base directory for RocksDB databases

// Helper function to get the full path for a DB instance
fn db_path(name: &str) -> String {
    format!("{}{}", DB_BASE_PATH, name)
}

// A generic wrapper around RocksDB for a specific mempool
pub struct MempoolDb<K, V>
where
    K: Serialize + DeserializeOwned + AsRef<[u8]>,
    V: Serialize + DeserializeOwned,
{
    db: DB,
    _phantom_key: PhantomData<K>,
    _phantom_value: PhantomData<V>,
}

impl<K, V> MempoolDb<K, V>
where
    K: Serialize + DeserializeOwned + AsRef<[u8]>,
    V: Serialize + DeserializeOwned,
{
    pub fn new(db_name: &str) -> Result<Self, RocksDbError> {
        let path = db_path(db_name);
        let mut opts = Options::default();
        opts.create_if_missing(true);
        match DB::open(&opts, &Path::new(&path)) {
            Ok(db) => Ok(MempoolDb {
                db,
                _phantom_key: PhantomData,
                _phantom_value: PhantomData,
            }),
            Err(e) => {
                eprintln!("Failed to open DB at path {}: {}", path, e);
                Err(e)
            }
        }
    }

    pub fn put(&self, key: &K, value: &V) -> Result<(), RocksDbError> {
        let key_bytes = serde_json::to_vec(key).map_err(|e| RocksDbError::new(e.to_string()))?;
        let value_bytes = serde_json::to_vec(value).map_err(|e| RocksDbError::new(e.to_string()))?;
        self.db.put(key_bytes, value_bytes)
    }

    pub fn get(&self, key: &K) -> Result<Option<V>, RocksDbError> {
        let key_bytes = serde_json::to_vec(key).map_err(|e| RocksDbError::new(e.to_string()))?;
        match self.db.get(key_bytes)? {
            Some(value_bytes) => {
                let value = serde_json::from_slice(&value_bytes).map_err(|e| RocksDbError::new(e.to_string()))?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    pub fn delete(&self, key: &K) -> Result<(), RocksDbError> {
        let key_bytes = serde_json::to_vec(key).map_err(|e| RocksDbError::new(e.to_string()))?;
        self.db.delete(key_bytes)
    }

    // Note: Iterating over a RocksDB instance can be complex.
    // For simplicity, we might load critical data into memory or use specific query patterns.
    // This basic iterator fetches all keys and values. Use with caution on large DBs.
    pub fn get_all(&self) -> Result<Vec<(K, V)>, RocksDbError> {
        let mut results = Vec::new();
        let iter = self.db.iterator(rocksdb::IteratorMode::Start);
        for item in iter {
            match item {
                Ok((key_bytes, value_bytes)) => {
                    let key = serde_json::from_slice(&key_bytes).map_err(|e| RocksDbError::new(e.to_string()))?;
                    let value = serde_json::from_slice(&value_bytes).map_err(|e| RocksDbError::new(e.to_string()))?;
                    results.push((key, value));
                }
                Err(e) => return Err(e),
            }
        }
        Ok(results)
    }
}

// This struct will hold all individual mempool DB instances.
pub struct AllMempoolDbs {
    // Define types based on data_structures.rs
    // pub raw_tx_mempool_db: MempoolDb<NodeId, HashMap<RawTxId, RawTransactionEntry>>,
    // pub validation_tasks_mempool_db: MempoolDb<RawTxId, Vec<ValidationTaskItem>>,
    // pub locked_utxo_mempool_db: MempoolDb<UtxoId, i64>,
    // pub processing_tx_mempool_db: MempoolDb<TxId, ProcessingTransactionEntry>,
    // pub tx_mempool_db: MempoolDb<TxId, FinalizedTransactionEntry>,
    // pub uptime_mempool_db: MempoolDb<NodeId, UptimeEntry>,
    // For simplicity in initialization, we'll use more generic types for now,
    // assuming keys are strings and values are JSON-serializable structs.
    // Specific types can be enforced at the application logic layer.
    pub raw_tx_mempool_db: MempoolDb<String, String>, // NodeId -> JSON string of HashMap<RawTxId, RawTransactionEntry>
    pub validation_tasks_mempool_db: MempoolDb<String, String>, // RawTxId -> JSON string of Vec<ValidationTaskItem>
    pub locked_utxo_mempool_db: MempoolDb<String, i64>, // UtxoId -> lock_timestamp
    pub processing_tx_mempool_db: MempoolDb<String, String>, // TxId -> JSON string of ProcessingTransactionEntry
    pub tx_mempool_db: MempoolDb<String, String>, // TxId -> JSON string of FinalizedTransactionEntry
    pub uptime_mempool_db: MempoolDb<String, String>, // NodeId -> JSON string of UptimeEntry
}

impl AllMempoolDbs {
    pub fn new() -> Result<Self, RocksDbError> {
        // Ensure the base directory exists
        std::fs::create_dir_all(DB_BASE_PATH).map_err(|e| RocksDbError::new(format!("Failed to create base DB directory: {}", e)))?;

        Ok(AllMempoolDbs {
            raw_tx_mempool_db: MempoolDb::new("raw_tx_mempool")?,
            validation_tasks_mempool_db: MempoolDb::new("validation_tasks_mempool")?,
            locked_utxo_mempool_db: MempoolDb::new("locked_utxo_mempool")?,
            processing_tx_mempool_db: MempoolDb::new("processing_tx_mempool")?,
            tx_mempool_db: MempoolDb::new("tx_mempool")?,
            uptime_mempool_db: MempoolDb::new("uptime_mempool")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_structures::{TransactionData, RawTransactionEntry, ValidationTaskItem, NodeId, RawTxId};
    use std::collections::HashMap;
    use tempfile::tempdir;

    // Helper to create a temporary path for DBs during tests
    fn setup_test_db(db_name: &str) -> MempoolDb<String, String> {
        let dir = tempdir().unwrap();
        let path = dir.path().join(db_name);
        let mut opts = Options::default();
        opts.create_if_missing(true);
        let db = DB::open(&opts, &path).unwrap();
        MempoolDb {
            db,
            _phantom_key: PhantomData,
            _phantom_value: PhantomData,
        }
    }

    #[test]
    fn test_mempool_db_operations() {
        // It's tricky to test AllMempoolDbs::new() directly because it creates actual directories.
        // We'll test MempoolDb<K,V> with a temporary directory.
        let test_db_name = "test_ops_db";
        let temp_dir = tempdir().unwrap();
        let db_path_str = temp_dir.path().join(test_db_name).to_str().unwrap().to_string();

        // Create a MempoolDb instance for testing
        let mut opts = Options::default();
        opts.create_if_missing(true);
        let db_instance = DB::open(&opts, &db_path_str).unwrap();
        let mempool_db: MempoolDb<String, RawTransactionEntry> = MempoolDb {
            db: db_instance,
            _phantom_key: PhantomData,
            _phantom_value: PhantomData,
        };

        let mut to_map = HashMap::new();
        to_map.insert("bob_address".to_string(), 1.0);
        let mut from_map = HashMap::new();
        from_map.insert("alice_utxo1".to_string(), 2.0);
        let tx_data = TransactionData {
            to: to_map,
            from: from_map,
            user: "alice_address".to_string(),
            sig: Some("alice_signature".to_string()),
            stake: 0.2,
            fee: 0.1,
        };
        let validation_task = ValidationTaskItem { task_id: "task1".to_string(), complete: false, assigned_by_leader_id: "leader1".to_string() };
        let raw_tx_entry = RawTransactionEntry {
            tx_data,
            validation_timestamps: vec![12345],
            validation_tasks: vec![validation_task],
            tx_timestamp: 67890,
        };

        let key: RawTxId = "test_raw_tx_id_123".to_string();

        // Test put
        mempool_db.put(&key, &raw_tx_entry).unwrap();

        // Test get
        let retrieved_entry = mempool_db.get(&key).unwrap().unwrap();
        assert_eq!(retrieved_entry.tx_timestamp, raw_tx_entry.tx_timestamp);
        assert_eq!(retrieved_entry.tx_data, raw_tx_entry.tx_data);
        assert_eq!(retrieved_entry.validation_tasks, raw_tx_entry.validation_tasks);


        // Test get_all
        let all_entries = mempool_db.get_all().unwrap();
        assert_eq!(all_entries.len(), 1);
        assert_eq!(all_entries[0].0, key);
        assert_eq!(all_entries[0].1.tx_data, raw_tx_entry.tx_data);

        // Test delete
        mempool_db.delete(&key).unwrap();
        let should_be_none = mempool_db.get(&key).unwrap();
        assert!(should_be_none.is_none());

        // Clean up test directory (tempdir does this automatically on drop)
    }

    #[test]
    fn test_all_mempool_dbs_new_path_creation() {
        // This test will attempt to create the ./db_data/ directory structure.
        // It's an integration-style test for db path creation.
        // We should clean up after if possible, or ensure tests can run multiple times.
        let base_path = Path::new(DB_BASE_PATH);
        if base_path.exists() {
            std::fs::remove_dir_all(base_path).unwrap_or_else(|e| eprintln!("Pre-test cleanup failed: {}", e));
        }

        let dbs = AllMempoolDbs::new();
        assert!(dbs.is_ok(), "AllMempoolDbs::new() should succeed");
        assert!(base_path.exists(), "Base DB directory should be created");
        assert!(base_path.join("raw_tx_mempool").exists());
        assert!(base_path.join("validation_tasks_mempool").exists());
        assert!(base_path.join("locked_utxo_mempool").exists());
        assert!(base_path.join("processing_tx_mempool").exists());
        assert!(base_path.join("tx_mempool").exists());
        assert!(base_path.join("uptime_mempool").exists());

        // Cleanup
        if base_path.exists() {
            std::fs::remove_dir_all(base_path).unwrap_or_else(|e| eprintln!("Post-test cleanup failed: {}", e));
        }
    }
}
