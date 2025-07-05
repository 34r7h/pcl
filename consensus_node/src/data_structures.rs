use serde::{Deserialize, Serialize};
use ed25519_dalek::{PublicKey, Signature, Verifier, Signer, Keypair};
use sha2::{Sha256, Digest};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

// Helper function to serialize Keypair
fn serialize_keypair<S>(keypair: &Option<Keypair>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    match keypair {
        Some(kp) => {
            let bytes = kp.to_bytes();
            serializer.serialize_some(&bytes.to_vec())
        }
        None => serializer.serialize_none(),
    }
}

// Helper function to deserialize Keypair
fn deserialize_keypair<'de, D>(deserializer: D) -> Result<Option<Keypair>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt_bytes: Option<Vec<u8>> = Option::deserialize(deserializer)?;
    match opt_bytes {
        Some(bytes) => {
            Keypair::from_bytes(&bytes)
                .map(Some)
                .map_err(serde::de::Error::custom)
        }
        None => Ok(None),
    }
}


#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TxData {
    pub to: HashMap<String, u64>, // bob_address: amount
    pub from: HashMap<String, u64>, // alice_utxo1: amount
    pub user: String, // alice_address (PublicKey hex)
    #[serde(with = "serde_bytes")]
    pub signature_bytes: Vec<u8>, // alice_signature (of TxData without this field)
    pub stake: f64,
    pub fee: f64,
    pub timestamp: DateTime<Utc>, // Timestamp of transaction creation by user
}

impl TxData {
    // Method to sign the transaction data (excluding the signature itself)
    pub fn sign(mut self, keypair: &Keypair) -> Self {
        self.signature_bytes = Vec::new(); // Clear signature for signing
        let message = serde_json::to_vec(&self).unwrap();
        let signature = keypair.sign(&message);
        self.signature_bytes = signature.to_bytes().to_vec();
        self
    }

    // Method to verify the transaction signature
    pub fn verify_signature(&self, user_public_key: &PublicKey) -> bool {
        let mut data_to_verify = self.clone();
        data_to_verify.signature_bytes = Vec::new(); // Clear signature for verification
        let message = serde_json::to_vec(&data_to_verify).unwrap();
        let signature = match Signature::from_bytes(&self.signature_bytes) {
            Ok(s) => s,
            Err(_) => return false,
        };
        user_public_key.verify(&message, &signature).is_ok()
    }

    // Method to calculate the hash of the transaction data (raw_tx_id)
    pub fn calculate_hash(&self) -> String {
        let mut hasher = Sha256::new();
        // Ensure a consistent serialization format for hashing
        let serialized_tx = serde_json::to_string(&self).unwrap_or_default();
        hasher.update(serialized_tx);
        format!("{:x}", hasher.finalize())
    }

    // Helper for testing: creates a new signed TxData
    pub fn new_dummy_signed(
        from_user_pk_hex: String, // Alice's public key hex
        to_user_pk_hex: String,   // Bob's public key hex
        amount_to_send: u64,
        from_utxo_amount: u64,
        stake: f64,
        fee: f64,
    ) -> (Self, Keypair) { // Returns TxData and the keypair used to sign it (Alice's keypair)
        let mut csprng = rand::rngs::OsRng{};
        let alice_keypair = Keypair::generate(&mut csprng);
        let alice_public_key_hex_check = hex::encode(alice_keypair.public.to_bytes());

        // Ensure the provided from_user_pk_hex matches the generated one if we want to use a specific identity.
        // For a true dummy, we can just use the generated keypair's public key.
        // If from_user_pk_hex is meant to be *the* identity, then the keypair should be derived from a seed or loaded.
        // For this dummy function, we'll assume from_user_pk_hex is just for the `user` field,
        // and we generate a new keypair for signing.
        // A more realistic scenario would involve looking up Alice's keypair.
        // For this test, we'll use the generated keypair's public key as the 'user'.

        let actual_from_user_pk_hex = alice_public_key_hex_check; // Use generated key's PK

        let from_utxo_id = format!("dummy_utxo_for_{}", actual_from_user_pk_hex);

        let tx = TxData {
            to: HashMap::from([(to_user_pk_hex, amount_to_send)]),
            from: HashMap::from([(from_utxo_id, from_utxo_amount)]),
            user: actual_from_user_pk_hex.clone(),
            signature_bytes: vec![], // Will be filled by sign method
            stake,
            fee,
            timestamp: Utc::now(),
        };
        (tx.sign(&alice_keypair), alice_keypair)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ValidationTaskType {
    UserSignatureAndBalanceValidation, // Alice validates her own TX sig and UTXOs
    LeaderTimestampMathCheck,        // Validators check Charlie's timestamp averaging
    DltFinalityCheck,                // DLT specific, e.g. digital root for XMBL
    // Potentially other types
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ValidationTask {
    pub task_id: String, // Unique ID for this task instance (e.g., hash of details or UUID)
    pub task_type: ValidationTaskType,
    pub raw_tx_id: String,     // The raw transaction this task pertains to
    pub subject_tx_id: String, // The transaction ID this task is about (can be raw_tx_id or processed_tx_id)

    pub assigned_by_leader_id: Option<String>, // Public key of the leader who decided this task should be done (e.g. Charlie)
    pub generated_by_leader_id: String, // Public key of the leader who created/defined this specific task (e.g. L2, L8)

    pub assigned_to_user_pk_hex: Option<String>, // Public key of the user if the task is for them (Alice)
    // pub assigned_to_validator_pk_hex: Option<String>, // Public key of a validator if task is for network validators

    pub completed: bool,
    // For user tasks, this is Alice's signature on {task_id + raw_tx_id + completion_timestamp}
    // For validator tasks, this is validator's signature
    pub completion_signature_bytes: Option<Vec<u8>>,
    pub completion_timestamp: Option<DateTime<Utc>>,
    pub completion_reported_to_origin_leader: bool, // True if L2/L8 reported Alice's completion to Charlie
}

impl ValidationTask {
    pub fn new(
        task_type: ValidationTaskType,
        raw_tx_id: String,
        subject_tx_id: String,
        generated_by_leader_id: String,
        assigned_to_user_pk_hex: Option<String>,
    ) -> Self {
        let task_id_material = format!(
            "{:?}-{}-{}-{}-{}",
            task_type, raw_tx_id, subject_tx_id, generated_by_leader_id, Utc::now().timestamp_nanos()
        );
        let mut hasher = Sha256::new();
        hasher.update(task_id_material);
        let task_id = format!("task_{:x}", hasher.finalize());

        ValidationTask {
            task_id,
            task_type,
            raw_tx_id,
            subject_tx_id,
            assigned_by_leader_id: None, // Charlie will fill this when he officially assigns
            generated_by_leader_id,
            assigned_to_user_pk_hex,
            completed: false,
            completion_signature_bytes: None,
            completion_timestamp: None,
            completion_reported_to_origin_leader: false,
        }
    }

    pub fn sign_completion(&mut self, keypair: &Keypair, timestamp: DateTime<Utc>) {
        let message = format!("{}{}{}", self.task_id, self.raw_tx_id, timestamp.to_rfc3339());
        let signature = keypair.sign(message.as_bytes());
        self.completion_signature_bytes = Some(signature.to_bytes().to_vec());
        self.completion_timestamp = Some(timestamp);
        self.completed = true;
    }

    pub fn verify_completion_signature(&self, signer_public_key: &PublicKey) -> bool {
        if !self.completed || self.completion_signature_bytes.is_none() || self.completion_timestamp.is_none() {
            return false;
        }
        let signature_bytes = self.completion_signature_bytes.as_ref().unwrap();
        let timestamp = self.completion_timestamp.unwrap();
        let message = format!("{}{}{}", self.task_id, self.raw_tx_id, timestamp.to_rfc3339());

        match Signature::from_bytes(signature_bytes) {
            Ok(sig) => signer_public_key.verify(message.as_bytes(), &sig).is_ok(),
            Err(_) => false,
        }
    }
}


#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RawTxMempoolEntry {
    pub tx_data: TxData,
    pub validation_timestamps: Vec<DateTime<Utc>>,
    pub validation_tasks: HashMap<String, ValidationTask>, // leader_id -> Vec<ValidationTask>
    pub tx_receive_timestamp: DateTime<Utc>, // When leader first received this tx
    pub leader_id: String, // Public key of the leader node that created this entry
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProcessingTxMempoolEntry {
    pub tx_data: TxData, // Original TxData
    pub averaged_validation_timestamp: DateTime<Utc>,
    #[serde(with = "serde_bytes")]
    pub leader_signature_bytes: Vec<u8>, // Leader's signature on {averaged_timestamp + tx_data_hash}
    pub leader_id: String, // Public key of the leader who processed this
    pub tx_id: String, // Hash of {averaged_validation_timestamp + tx_data_hash}
}

impl ProcessingTxMempoolEntry {
     // Method for leader to sign the processing entry
    pub fn sign(mut self, keypair: &Keypair) -> Self {
        let message_to_sign = format!("{}{}", self.averaged_validation_timestamp.to_rfc3339(), self.tx_data.calculate_hash());
        let signature = keypair.sign(message_to_sign.as_bytes());
        self.leader_signature_bytes = signature.to_bytes().to_vec();
        self
    }

    // Method to verify the leader's signature
    pub fn verify_leader_signature(&self, leader_public_key: &PublicKey) -> bool {
        let message_to_verify = format!("{}{}", self.averaged_validation_timestamp.to_rfc3339(), self.tx_data.calculate_hash());
        let signature = match Signature::from_bytes(&self.leader_signature_bytes) {
            Ok(s) => s,
            Err(_) => return false,
        };
        leader_public_key.verify(message_to_verify.as_bytes(), &signature).is_ok()
    }
}


#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UptimeMempoolEntry {
    // Store as node_id (IP or PublicKey) -> (last_pulse_timestamp, pulse_count, average_response_time_ms)
    // For simplicity, using String as key for PublicKey hex or IP address.
    // pub pulses: HashMap<String, (DateTime<Utc>, u64, u64)>,
    pub last_pulse_timestamp: DateTime<Utc>,
    pub pulse_count: u64,
    pub total_response_time_ms: u64, // Sum of all response times
    // average_response_time_ms = total_response_time_ms / pulse_count
}

impl UptimeMempoolEntry {
    pub fn new(timestamp: DateTime<Utc>, initial_response_time_ms: u64) -> Self {
        UptimeMempoolEntry {
            last_pulse_timestamp: timestamp,
            pulse_count: 1,
            total_response_time_ms: initial_response_time_ms,
        }
    }

    pub fn record_pulse(&mut self, timestamp: DateTime<Utc>, response_time_ms: u64) {
        self.last_pulse_timestamp = timestamp;
        self.pulse_count += 1;
        self.total_response_time_ms += response_time_ms;
    }

    pub fn get_average_response_time_ms(&self) -> f64 {
        if self.pulse_count == 0 {
            0.0
        } else {
            self.total_response_time_ms as f64 / self.pulse_count as f64
        }
    }
}


#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NodeIdentity {
    #[serde(serialize_with = "serialize_keypair", deserialize_with = "deserialize_keypair")]
    pub keypair: Option<Keypair>, // Option to allow for transient identities or loading later
    pub public_key_hex: String,
}

impl NodeIdentity {
    pub fn new() -> Self {
        let mut csprng = rand::rngs::OsRng{};
        let keypair = Keypair::generate(&mut csprng);
        let public_key_hex = hex::encode(keypair.public.to_bytes());
        NodeIdentity {
            keypair: Some(keypair),
            public_key_hex,
        }
    }

    pub fn public_key(&self) -> Option<PublicKey> {
        self.keypair.as_ref().map(|kp| kp.public)
    }

    pub fn sign_message(&self, message: &[u8]) -> Option<Signature> {
        self.keypair.as_ref().map(|kp| kp.sign(message))
    }
}

// Mock structure for final blockchain state (balances)
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct BlockchainState {
    // user_address (PublicKey hex) -> UTXO ID -> amount
    pub user_balances: HashMap<String, HashMap<String, u64>>,
    pub latest_block_hash: Option<String>,
    pub block_height: u64,
}

// Main mempool collections (to be managed by the node)
// These would typically be wrappers around RocksDB instances.
// For now, they are placeholders.
pub struct Mempools {
    // pub raw_tx_mempool: RocksDB, // Key: raw_tx_id, Value: RawTxMempoolEntry
    // pub validation_tasks_mempool: RocksDB, // Key: task_id or raw_tx_id, Value: ValidationTask or Vec<ValidationTask>
    // pub locked_utxo_mempool: RocksDB, // Key: utxo_id, Value: tx_id (that locked it)
    // pub processing_tx_mempool: RocksDB, // Key: tx_id, Value: ProcessingTxMempoolEntry
    // pub tx_mempool: RocksDB, // Key: tx_id (finalized for block inclusion by DLT), Value: TxData or some DLT specific format
    // pub uptime_mempool: RocksDB, // Key: node_id (e.g. PublicKey hex), Value: UptimeMempoolEntry
}

// Example of how a message might look on the P2P network
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct LeaderCandidate {
    pub peer_id_str: String, // Libp2p PeerId as string
    pub node_public_key_hex: String, // Application-level public key for identity
    pub uptime_score: u64, // Higher is better (e.g., total uptime seconds or pulse count)
    pub response_time_score: u64, // Lower is better (e.g., average ms, inverted for scoring if needed)
    pub combined_score: u64, // Calculated score for ranking
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LeaderElectionVote {
    pub candidate_node_public_key_hex: String, // The candidate being voted for
    pub round: u8,
    pub voter_node_public_key_hex: String, // The public key of the voter
    #[serde(with = "serde_bytes")]
    pub voter_signature: Vec<u8>, // Signature of {candidate_node_public_key_hex + round}
}

impl LeaderElectionVote {
    pub fn sign(mut self, keypair: &Keypair) -> Self {
        let message = format!("{}{}", self.candidate_node_public_key_hex, self.round);
        let signature = keypair.sign(message.as_bytes());
        self.voter_signature = signature.to_bytes().to_vec();
        self
    }

    pub fn verify_signature(&self, public_key: &PublicKey) -> bool {
        let message = format!("{}{}", self.candidate_node_public_key_hex, self.round);
        let signature = match Signature::from_bytes(&self.voter_signature) {
            Ok(s) => s,
            Err(_) => return false,
        };
        public_key.verify(message.as_bytes(), &signature).is_ok()
    }
}


#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum P2PMessage {
    Pulse,
    PulseResponse { original_timestamp: DateTime<Utc> },
    // Broadcasts this node's full uptime mempool (or relevant parts for leader election)
    // Key is the observed node's public_key_hex, Value is their UptimeMempoolEntry
    UptimeDataBroadcast(HashMap<String, UptimeMempoolEntry>),
    // A node broadcasts its nominations for leaders after processing UptimeDataBroadcasts
    LeaderNominations {
        nominations: Vec<LeaderCandidate>, // List of candidates this node nominates
        nominator_node_public_key_hex: String, // PK of the nominator
    },
    LeaderElectionVoteMsg(LeaderElectionVote),
    NewLeaderList {
        leaders: Vec<String>, // Sorted list of leader node_public_key_hex
        list_hash: String, // Hash of the sorted leader list
        effective_from_timestamp: DateTime<Utc>, // When this leader list becomes active
    },
    RawTransactionGossip(Box<RawTxMempoolEntry>),
    // Sent by a leader (e.g. L2) to the originating leader (Charlie) offering a task for a user (Alice)
    OfferValidationTaskToOriginLeader {
        task: ValidationTask, // The task L2 generated for Alice, targeting raw_tx_id
        // raw_tx_id is already in task struct
        // target_origin_leader_id is implicit (Charlie's pubkey)
    },
    // Sent by Charlie to Alice (conceptual: might be broadcast or Alice polls for it)
    // This message would contain tasks Charlie has approved for Alice from various offers.
    ValidationTaskAssignmentToUser {
        tasks_for_user: Vec<ValidationTask>, // List of tasks Alice needs to do
        user_public_key_hex: String, // Alice's PK
        raw_tx_id: String, // The specific raw_tx_id these tasks are for
    },
    // Sent by Alice (user) to the leader who *generated* the task (e.g. L2)
    UserValidationTaskCompletion {
        task_id: String,
        raw_tx_id: String,
        user_public_key_hex: String, // Alice's PK
        #[serde(with = "serde_bytes")]
        completion_signature_bytes: Vec<u8>, // Alice's signature on {task_id + raw_tx_id + completion_timestamp}
        completion_timestamp: DateTime<Utc>,
    },
    // Sent by a task-generating leader (e.g. L2) to the originating leader (Charlie)
    // after Alice has reported completion of a task to L2.
    ForwardUserTaskCompletionToOriginLeader {
        completed_task: ValidationTask, // The task, now marked completed with Alice's signature and timestamp
    },
    // Sent by a validator after completing Charlie's math check for a ProcessingTxMempoolEntry
    VerifiedProcessingTxBroadcast {
        processing_entry: ProcessingTxMempoolEntry, // The entry they verified
        validator_id_pk_hex: String, // Validator's PubKeyHex
        #[serde(with = "serde_bytes")]
        validator_signature_on_tx_id: Vec<u8>, // Validator signs the processing_entry.tx_id
    },
    ProcessingTransactionGossip(Box<ProcessingTxMempoolEntry>), // Used by leaders after finality checks
    TransactionInvalidationNotice { tx_id: String, reason: String },
}


// --- Digital Root Calculation ---
// Helper for calculating digital root of a sum.
fn calculate_digital_root_of_sum(mut n: u32) -> u32 {
    if n == 0 { return 0; }
    while n >= 10 {
        n = n.to_string().chars().filter_map(|c| c.to_digit(10)).sum();
    }
    n
}

// Calculates a sum from a hex string (tx_id) then its digital root.
pub fn calculate_digital_root_of_hex_string(hex_str: &str) -> u32 {
    let mut sum_val: u32 = 0;
    for char_hex in hex_str.chars() {
        if let Some(val) = char_hex.to_digit(16) { // base 16 for hex
            sum_val += val;
        }
    }
    calculate_digital_root_of_sum(sum_val)
}
