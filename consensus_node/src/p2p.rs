use libp2p::{
    core::upgrade,
    futures::StreamExt,
    gossipsub::{self, GossipsubEvent, GossipsubMessage, IdentTopic as Topic, MessageAuthenticity, ValidationMode},
    identity,
    mdns::{Mdns, MdnsEvent, Config as MdnsConfig},
    noise,
    swarm::{NetworkBehaviourEventProcess, SwarmBuilder, SwarmEvent, Swarm},
    tcp::{Config as TcpConfig, TokioTcpTransport},
    yamux, Multiaddr, PeerId, Transport,
    NetworkBehaviour,
    gossipsub::IdentTopic
};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::Duration;
use tokio::{select, time::interval, sync::{Mutex, mpsc}};
use crate::data_structures::{NodeIdentity, P2PMessage, UptimeMempoolEntry, LeaderCandidate, LeaderElectionVote, TxData, RawTxMempoolEntry};
use std::collections::HashMap;
use chrono::{Utc, DateTime};
use rocksdb::{DB, Options, IteratorMode, WriteBatch};
use serde_json;
use std::sync::Arc;
use sha2::{Sha256, Digest};
use ed25519_dalek::PublicKey;


const NUM_LEADERS_TO_ELECT: usize = 5;
const NUM_VOTING_ROUNDS: u8 = 3;
const UPTIME_BROADCAST_INTERVAL_SECS: u64 = 300; // 5 minutes, (was 2 hours)
const ELECTION_PHASE_TIMEOUT_SECS: u64 = 60; // Timeout for nomination/voting phases

// Define keys for different mempools/data types within RocksDB
const DB_RAW_TX_MEMPOOL_PREFIX: &str = "rawtx_";
const DB_VALIDATION_TASKS_MEMPOOL_PREFIX: &str = "valtask_";
const DB_LOCKED_UTXO_MEMPOOL_PREFIX: &str = "lockutxo_";
const DB_PROCESSING_TX_MEMPOOL_PREFIX: &str = "proctx_";
const DB_FINAL_TX_MEMPOOL_PREFIX: &str = "finaltx_"; // For step 6 tx_mempool
const DB_UPTIME_PREFIX: &str = "uptime_";

const MIN_VALIDATION_TIMESTAMPS_FOR_PROCESSING: usize = 1;
const NUM_LEADERS_FOR_VALIDATOR_BROADCAST: usize = 3;


// Define the network behaviour
#[derive(NetworkBehaviour)]
#[behaviour(event_process = true)]
pub struct ConsensusBehaviour {
    pub gossipsub: gossipsub::Gossipsub,
    pub mdns: Mdns,
    #[behaviour(ignore)]
    pub db: Arc<DB>,
    #[behaviour(ignore)]
    pub local_peer_id: PeerId,
    #[behaviour(ignore)]
    pub node_identity: Arc<NodeIdentity>,

    // Leader Election State
    #[behaviour(ignore)]
    pub current_leaders: Arc<Mutex<Vec<String>>>,
    #[behaviour(ignore)]
    pub last_leader_list_hash: Arc<Mutex<Option<String>>>,
    #[behaviour(ignore)]
    pub received_uptime_data: Arc<Mutex<HashMap<String, HashMap<String, UptimeMempoolEntry>>>>,
    #[behaviour(ignore)]
    pub received_nominations: Arc<Mutex<HashMap<String, Vec<LeaderCandidate>>>>,
    #[behaviour(ignore)]
    pub election_round: Arc<Mutex<u8>>,
    #[behaviour(ignore)]
    pub votes_for_round: Arc<Mutex<HashMap<u8, HashMap<String, Vec<LeaderElectionVote>>>>>,
    #[behaviour(ignore)]
    pub election_in_progress: Arc<Mutex<bool>>,
    #[behaviour(ignore)]
    pub last_uptime_broadcast_time: Arc<Mutex<Option<DateTime<Utc>>>>,
    #[behaviour(ignore)]
    pub election_phase_start_time: Arc<Mutex<Option<DateTime<Utc>>>>,

    // Channel for gossiped transactions to be processed by the main loop
    #[behaviour(ignore)]
    pub gossiped_tx_receiver: mpsc::Receiver<RawTxMempoolEntry>,
    #[behaviour(ignore)]
    pub gossiped_tx_sender: mpsc::Sender<RawTxMempoolEntry>,

    // New state for Validation Task Workflow (Steps 3 & 4)
    #[behaviour(ignore)]
    // Key: raw_tx_id, Value: Vec of tasks offered by other leaders for this tx
    pub offered_validation_tasks: Arc<Mutex<HashMap<String, Vec<ValidationTask>>>>,
    #[behaviour(ignore)]
    // Key: raw_tx_id (of Alice's tx), Value: Vec of tasks Charlie assigned to Alice
    // This also serves as Charlie's record of what he expects Alice to complete.
    pub tasks_assigned_to_users: Arc<Mutex<HashMap<String, Vec<ValidationTask>>>>,

    // Channels for new message types to be processed by the main loop
    #[behaviour(ignore)]
    pub offer_val_task_receiver: mpsc::Receiver<P2PMessage>, // For OfferValidationTaskToOriginLeader
    #[behaviour(ignore)]
    pub offer_val_task_sender: mpsc::Sender<P2PMessage>,
    #[behaviour(ignore)]
    pub user_task_completion_receiver: mpsc::Receiver<P2PMessage>, // For UserValidationTaskCompletion
    #[behaviour(ignore)]
    pub user_task_completion_sender: mpsc::Sender<P2PMessage>,
    #[behaviour(ignore)]
    pub forwarded_completion_receiver: mpsc::Receiver<P2PMessage>, // For ForwardUserTaskCompletionToOriginLeader
    #[behaviour(ignore)]
    pub forwarded_completion_sender: mpsc::Sender<P2PMessage>,

    // Test channel to simulate Alice sending a completion
    #[behaviour(ignore)]
    pub simulate_alice_completion_sender: mpsc::Sender<P2PMessage>,
    #[behaviour(ignore)]
    pub simulate_alice_completion_receiver: mpsc::Receiver<P2PMessage>,

    // Channel for Step 6: Validator broadcast of verified ProcessingTX
    #[behaviour(ignore)]
    pub verified_processing_tx_receiver: mpsc::Receiver<P2PMessage>,
    #[behaviour(ignore)]
    pub verified_processing_tx_sender: mpsc::Sender<P2PMessage>,

    // Channel for Invalidation Notices
    #[behaviour(ignore)]
    pub invalidation_notice_receiver: mpsc::Receiver<P2PMessage>,
    #[behaviour(ignore)]
    pub invalidation_notice_sender: mpsc::Sender<P2PMessage>,

    // Channel for transactions submitted by clients/simulators
    #[behaviour(ignore)]
    pub client_submitted_tx_receiver: mpsc::Receiver<TxData>,
    #[behaviour(ignore)]
    pub client_submitted_tx_sender: mpsc::Sender<TxData>,
}

impl ConsensusBehaviour {
    fn db(&self) -> Arc<DB> {
        self.db.clone()
    }

    fn store_uptime_entry(&self, peer_id_str: &str, entry: &UptimeMempoolEntry) {
        let db_key = format!("{}{}",DB_UPTIME_PREFIX, peer_id_str);
        match serde_json::to_string(entry) {
            Ok(json_entry) => {
                if let Err(e) = self.db().put(db_key, json_entry) {
                    eprintln!("Failed to store uptime entry for {}: {}", peer_id_str, e);
                }
            }
            Err(e) => eprintln!("Failed to serialize uptime entry: {}", e),
        }
    }

    fn get_uptime_entry(&self, peer_id_str: &str) -> Option<UptimeMempoolEntry> {
        let db_key = format!("{}{}",DB_UPTIME_PREFIX, peer_id_str);
        match self.db().get(db_key) {
            Ok(Some(value_bytes)) => serde_json::from_slice(&value_bytes).ok(),
            Ok(None) => None,
            Err(e) => {
                eprintln!("Failed to retrieve uptime entry for {}: {}", peer_id_str, e);
                None
            }
        }
    }

    fn get_all_local_uptime_data(&self) -> HashMap<String, UptimeMempoolEntry> {
        let mut all_uptime_data = HashMap::new();
        let iterator = self.db().iterator(IteratorMode::Start);
        for item in iterator {
            if let Ok((key_bytes, value_bytes)) = item {
                let key_str = String::from_utf8_lossy(&key_bytes);
                if key_str.starts_with(DB_UPTIME_PREFIX) {
                    let Pk_key = key_str.trim_start_matches(DB_UPTIME_PREFIX).to_string();
                    if let Ok(entry) = serde_json::from_slice::<UptimeMempoolEntry>(&value_bytes) {
                        all_uptime_data.insert(Pk_key, entry);
                    }
                }
            }
        }
        all_uptime_data
    }

    async fn prune_expired_uptime_entries(&mut self) {
        let sixty_seconds_ago = Utc::now() - chrono::Duration::seconds(60);
        let mut entries_to_remove = Vec::new();
        let db = self.db();
        let iter = db.iterator(IteratorMode::Start);
        for item in iter {
            if let Ok((key_bytes, value_bytes)) = item {
                if let Ok(key_str) = String::from_utf8(key_bytes.clone().into_vec()) {
                    if key_str.starts_with(DB_UPTIME_PREFIX) {
                        if let Ok(entry) = serde_json::from_slice::<UptimeMempoolEntry>(&value_bytes){
                            if entry.last_pulse_timestamp < sixty_seconds_ago {
                                entries_to_remove.push(key_bytes.into_vec());
                                println!("Pruning expired uptime entry for key: {}", key_str);
                            }
                        }
                    }
                }
            }
        }
        for key in entries_to_remove {
            if let Err(e) = db.delete(&key) {
                eprintln!("Failed to delete expired uptime entry: {:?}", e);
            }
        }
    }

    async fn broadcast_uptime_data(&mut self) {
        let local_uptime_data = self.get_all_local_uptime_data();
        if local_uptime_data.is_empty() { return; }
        let message = P2PMessage::UptimeDataBroadcast(local_uptime_data);
        match serde_json::to_vec(&message) {
            Ok(serialized_message) => {
                if self.gossipsub.publish(IdentTopic::new("consensus-messages"), serialized_message).is_ok() {
                    *self.last_uptime_broadcast_time.lock().await = Some(Utc::now());
                    if !*self.election_in_progress.lock().await { self.start_nomination_phase().await; }
                } else { eprintln!("Error publishing UptimeDataBroadcast"); }
            }
            Err(e) => eprintln!("Failed to serialize UptimeDataBroadcast: {}", e),
        }
    }

    async fn start_nomination_phase(&mut self) {
        let mut election_prog = self.election_in_progress.lock().await;
        if *election_prog { return; }
        *election_prog = true;
        *self.election_phase_start_time.lock().await = Some(Utc::now());
        *self.election_round.lock().await = 0;
        self.received_uptime_data.lock().await.clear();
        self.received_nominations.lock().await.clear();
        self.votes_for_round.lock().await.clear();
        println!("Leader election: Nomination phase started.");
    }

    async fn process_received_uptime_data(&mut self) {
        let mut election_round = self.election_round.lock().await;
        if *election_round != 0 || !*self.election_in_progress.lock().await { return; }
        let phase_start = self.election_phase_start_time.lock().await.unwrap_or(Utc::now());
        if Utc::now().signed_duration_since(phase_start).num_seconds() < ELECTION_PHASE_TIMEOUT_SECS as i64 { return; }

        *election_round = 1;
        *self.election_phase_start_time.lock().await = Some(Utc::now());
        let all_uptime = self.received_uptime_data.lock().await;
        if all_uptime.is_empty() {
            *self.election_in_progress.lock().await = false; return;
        }
        let mut aggregated_uptime: HashMap<String, Vec<UptimeMempoolEntry>> = HashMap::new();
        for (_, uptime_map) in all_uptime.iter() {
            for (pk, entry) in uptime_map.iter() {
                aggregated_uptime.entry(pk.clone()).or_default().push(entry.clone());
            }
        }
        let mut candidates: Vec<LeaderCandidate> = Vec::new();
        for (pk, entries) in aggregated_uptime {
            if entries.is_empty() { continue; }
            let total_pulses: u64 = entries.iter().map(|e| e.pulse_count).sum();
            let total_rtt: u64 = entries.iter().map(|e| e.total_response_time_ms).sum();
            if total_pulses == 0 { continue; }
            let avg_rtt = total_rtt / total_pulses;
            candidates.push(LeaderCandidate {
                peer_id_str: String::new(), node_public_key_hex: pk.clone(),
                uptime_score: total_pulses, response_time_score: if avg_rtt > 0 { 1_000_000 / avg_rtt } else { 0 },
                combined_score: total_pulses + (if avg_rtt > 0 { 1_000_000 / avg_rtt } else { 0 }),
            });
        }
        candidates.sort_by(|a, b| b.combined_score.cmp(&a.combined_score));
        let nominations = candidates.into_iter().take(NUM_LEADERS_TO_ELECT * 2).collect::<Vec<_>>();
        if nominations.is_empty() { *self.election_in_progress.lock().await = false; return; }
        let message = P2PMessage::LeaderNominations {
            nominations, nominator_node_public_key_hex: self.node_identity.public_key_hex.clone(),
        };
        if let Ok(s) = serde_json::to_vec(&message) {
            if self.gossipsub.publish(IdentTopic::new("consensus-messages"), s).is_err() {
                eprintln!("Failed to publish leader nominations");
            }
        }
    }

    async fn process_received_nominations(&mut self) {
        let current_round = *self.election_round.lock().await;
        if current_round != 1 || !*self.election_in_progress.lock().await { return; }
        let phase_start = self.election_phase_start_time.lock().await.unwrap_or(Utc::now());
        if Utc::now().signed_duration_since(phase_start).num_seconds() < ELECTION_PHASE_TIMEOUT_SECS as i64 { return; }

        let all_nominations = self.received_nominations.lock().await;
        if all_nominations.is_empty() { *self.election_in_progress.lock().await = false; return; }
        let mut counts: HashMap<LeaderCandidate, usize> = HashMap::new();
        for (_, list) in all_nominations.iter() {
            for cand in list { *counts.entry(cand.clone()).or_insert(0) += 1; }
        }
        let mut sorted_cands = counts.into_iter().collect::<Vec<_>>();
        sorted_cands.sort_by(|(ca, count_a), (cb, count_b)| {
            count_b.cmp(count_a).then_with(|| cb.combined_score.cmp(&ca.combined_score))
        });
        let cands_for_voting: Vec<LeaderCandidate> = sorted_cands.into_iter().map(|(c, _)| c).take(NUM_LEADERS_TO_ELECT * 2).collect();
        if cands_for_voting.is_empty() { *self.election_in_progress.lock().await = false; return; }
        self.cast_votes(1, cands_for_voting).await;
    }

    async fn cast_votes(&mut self, round: u8, candidates: Vec<LeaderCandidate>) {
        if self.node_identity.keypair.is_none() { return; }
        let keypair = self.node_identity.keypair.as_ref().unwrap();
        for (i, candidate) in candidates.iter().enumerate() {
            if i >= NUM_LEADERS_TO_ELECT { break; }
            let vote = LeaderElectionVote {
                candidate_node_public_key_hex: candidate.node_public_key_hex.clone(), round,
                voter_node_public_key_hex: self.node_identity.public_key_hex.clone(), voter_signature: vec![],
            }.sign(keypair);
            let message = P2PMessage::LeaderElectionVoteMsg(vote);
            if let Ok(s) = serde_json::to_vec(&message) {
                if self.gossipsub.publish(IdentTopic::new("consensus-messages"), s).is_err() {
                    eprintln!("Failed to publish vote for {}", candidate.node_public_key_hex);
                }
            }
        }
        *self.election_phase_start_time.lock().await = Some(Utc::now());
    }

    async fn process_received_votes(&mut self) {
        let mut current_round = self.election_round.lock().await;
        if *current_round == 0 || *current_round > NUM_VOTING_ROUNDS || !*self.election_in_progress.lock().await { return; }
        let phase_start = self.election_phase_start_time.lock().await.unwrap_or(Utc::now());
        if Utc::now().signed_duration_since(phase_start).num_seconds() < ELECTION_PHASE_TIMEOUT_SECS as i64 { return; }

        let all_votes = self.votes_for_round.lock().await;
        let votes_this_round = match all_votes.get(&*current_round) {
            Some(v) => v, None => { *self.election_in_progress.lock().await = false; return; }
        };
        if votes_this_round.is_empty() { *self.election_in_progress.lock().await = false; return; }
        let mut vote_counts: HashMap<String, usize> = HashMap::new();
        for (cand_pk, list) in votes_this_round.iter() { vote_counts.insert(cand_pk.clone(), list.len()); }
        let mut sorted_by_votes: Vec<(String, usize)> = vote_counts.into_iter().collect();
        sorted_by_votes.sort_by(|a, b| b.1.cmp(&a.1));

        if *current_round < NUM_VOTING_ROUNDS {
            let next_round_pks: Vec<String> = sorted_by_votes.iter()
                .take(NUM_LEADERS_TO_ELECT + (NUM_VOTING_ROUNDS - *current_round) as usize)
                .map(|(pk, _)| pk.clone()).collect();
            if next_round_pks.len() <= NUM_LEADERS_TO_ELECT && !next_round_pks.is_empty() {
                self.finalize_election(next_round_pks.iter().take(NUM_LEADERS_TO_ELECT).cloned().collect()).await;
            } else if next_round_pks.is_empty() { *self.election_in_progress.lock().await = false; }
            else {
                *current_round += 1;
                let dummy_cands = next_round_pks.iter().map(|pk| LeaderCandidate {
                    peer_id_str: "".to_string(), node_public_key_hex: pk.clone(), uptime_score:0, response_time_score:0, combined_score:0
                }).collect();
                self.cast_votes(*current_round, dummy_cands).await;
            }
        } else {
            let final_leaders: Vec<String> = sorted_by_votes.into_iter().take(NUM_LEADERS_TO_ELECT).map(|(pk, _)| pk).collect();
            self.finalize_election(final_leaders).await;
        }
    }

    async fn finalize_election(&mut self, leaders: Vec<String>) {
        if leaders.is_empty() { *self.election_in_progress.lock().await = false; return; }
        let mut sorted_leaders = leaders; sorted_leaders.sort();
        let mut hasher = Sha256::new();
        for pk in &sorted_leaders { hasher.update(pk.as_bytes()); }
        let list_hash = format!("{:x}", hasher.finalize());
        let message = P2PMessage::NewLeaderList {
            leaders: sorted_leaders.clone(), list_hash: list_hash.clone(), effective_from_timestamp: Utc::now(),
        };
        if let Ok(s) = serde_json::to_vec(&message) {
            if self.gossipsub.publish(IdentTopic::new("consensus-messages"), s).is_ok() {
                *self.current_leaders.lock().await = sorted_leaders;
                *self.last_leader_list_hash.lock().await = Some(list_hash);
            } else { eprintln!("Failed to publish new leader list"); }
        }
        *self.election_in_progress.lock().await = false;
        self.received_uptime_data.lock().await.clear();
        self.received_nominations.lock().await.clear();
        self.votes_for_round.lock().await.clear();
        *self.election_round.lock().await = 0;
    }

    // --- Transaction Workflow Step 1 & 2 ---
    pub async fn handle_incoming_raw_transaction(&mut self, tx_data: TxData) -> Result<(), String> {
        let current_leaders_lock = self.current_leaders.lock().await;
        if !current_leaders_lock.contains(&self.node_identity.public_key_hex) {
            return Err("Not a leader".to_string());
        }
        drop(current_leaders_lock); // Release lock

        let raw_tx_id = tx_data.calculate_hash();
        println!("Leader {} processing new raw transaction {} from user {}", self.node_identity.public_key_hex, raw_tx_id, tx_data.user);

        // Basic validation (e.g. signature) would happen here or before
        // For now, assume valid if it reaches here.

        let db = self.db();
        // Check if raw_tx_id already exists
        let raw_tx_db_key = format!("{}{}", DB_RAW_TX_MEMPOOL_PREFIX, raw_tx_id);
        if db.get(&raw_tx_db_key).map_err(|e| e.to_string())?.is_some() {
            return Err(format!("Transaction {} already processed", raw_tx_id));
        }

        // Check and lock UTXOs in a batch
        let mut batch = WriteBatch::default();
        for utxo_id in tx_data.from.keys() {
            let lock_key = format!("{}{}", DB_LOCKED_UTXO_MEMPOOL_PREFIX, utxo_id);
            if db.get(&lock_key).map_err(|e| e.to_string())?.is_some() {
                return Err(format!("UTXO {} already locked", utxo_id));
            }
            batch.put(&lock_key, raw_tx_id.as_bytes());
        }

        let entry = RawTxMempoolEntry {
            tx_data: tx_data.clone(),
            validation_timestamps: Vec::new(),
            validation_tasks: HashMap::new(),
            tx_receive_timestamp: Utc::now(),
            leader_id: self.node_identity.public_key_hex.clone(),
        };
        let json_entry = serde_json::to_string(&entry).map_err(|e| e.to_string())?;
        batch.put(&raw_tx_db_key, json_entry);

        let val_task_key = format!("{}{}", DB_VALIDATION_TASKS_MEMPOOL_PREFIX, raw_tx_id);
        batch.put(&val_task_key, Utc::now().to_rfc3339());

        db.write(batch).map_err(|e| e.to_string())?;
        println!("Atomically stored RawTx, locked UTXOs, and added to validation tasks for {}", raw_tx_id);

        let gossip_message = P2PMessage::RawTransactionGossip(Box::new(entry));
        if let Ok(serialized_message) = serde_json::to_vec(&gossip_message) {
            if self.gossipsub.publish(IdentTopic::new("consensus-messages"), serialized_message).is_err() {
                eprintln!("Error gossiping RawTransactionGossip for tx {}", raw_tx_id);
            }
        }
        Ok(())
    }

    pub async fn handle_gossiped_raw_transaction(&mut self, entry: RawTxMempoolEntry) -> Result<(), String> {
        let raw_tx_id = entry.tx_data.calculate_hash();
        if entry.leader_id == self.node_identity.public_key_hex {
            return Ok(()); // Ignore own gossip
        }
        println!("Node {} received gossiped RawTx {} from leader {}", self.node_identity.public_key_hex, raw_tx_id, entry.leader_id);

        let db = self.db();
        let raw_tx_db_key = format!("{}{}", DB_RAW_TX_MEMPOOL_PREFIX, raw_tx_id);
        if db.get(&raw_tx_db_key).map_err(|e| e.to_string())?.is_some() {
            return Ok(()); // Already have it
        }

        let mut batch = WriteBatch::default();
        for utxo_id in entry.tx_data.from.keys() {
            let lock_key = format!("{}{}", DB_LOCKED_UTXO_MEMPOOL_PREFIX, utxo_id);
            if let Some(existing_locker_bytes) = db.get(&lock_key).map_err(|e| e.to_string())? {
                if String::from_utf8_lossy(&existing_locker_bytes) != raw_tx_id {
                    return Err(format!("UTXO {} conflict for gossiped tx {}", utxo_id, raw_tx_id));
                }
            } else {
                 batch.put(&lock_key, raw_tx_id.as_bytes());
            }
        }

        let json_entry = serde_json::to_string(&entry).map_err(|e| e.to_string())?;
        batch.put(&raw_tx_db_key, json_entry);
        let val_task_key = format!("{}{}", DB_VALIDATION_TASKS_MEMPOOL_PREFIX, raw_tx_id);
        batch.put(&val_task_key, Utc::now().to_rfc3339());
        db.write(batch).map_err(|e| e.to_string())?;

        println!("Stored gossiped RawTx {} and associated data.", raw_tx_id);
        Ok(())
    }

    // Method for a leader (L2) to generate and offer a task for Alice's TX (raw_tx_id by Charlie)
    pub async fn generate_and_offer_task_for_raw_tx(&mut self, original_entry: &RawTxMempoolEntry) {
        let local_pk_hex = &self.node_identity.public_key_hex;
        let current_leaders_lock = self.current_leaders.lock().await;

        // Only leaders (not the originating one) should offer tasks initially.
        if !current_leaders_lock.contains(local_pk_hex) || *local_pk_hex == original_entry.leader_id {
            return;
        }
        drop(current_leaders_lock);

        println!("Leader {} is generating a task offer for raw_tx_id {} (originated by {} for user {}).",
            local_pk_hex, original_entry.tx_data.calculate_hash(), original_entry.leader_id, original_entry.tx_data.user);

        // For now, only UserSignatureAndBalanceValidation task
        let task = ValidationTask::new(
            ValidationTaskType::UserSignatureAndBalanceValidation,
            original_entry.tx_data.calculate_hash(),
            original_entry.tx_data.calculate_hash(), // subject_tx_id is raw_tx_id for this task type
            local_pk_hex.clone(), // This leader (L2) generated it
            Some(original_entry.tx_data.user.clone()) // Assigned to Alice (the user from TxData)
        );

        let message = P2PMessage::OfferValidationTaskToOriginLeader { task };

        if let Ok(serialized_message) = serde_json::to_vec(&message) {
            if self.gossipsub.publish(IdentTopic::new("consensus-messages"), serialized_message).is_err() {
                eprintln!("Error gossiping OfferValidationTaskToOriginLeader for raw_tx_id {}", original_entry.tx_data.calculate_hash());
            } else {
                println!("Leader {} gossiped OfferValidationTaskToOriginLeader for raw_tx_id {}.", local_pk_hex, original_entry.tx_data.calculate_hash());
            }
        }
    }

    // Method for Charlie (originating leader) to handle an offered task
    pub async fn handle_offered_validation_task(&mut self, task_offer: ValidationTask) {
        let local_pk_hex = &self.node_identity.public_key_hex;
        let raw_tx_id = task_offer.raw_tx_id.clone();

        let raw_tx_db_key = format!("{}{}", DB_RAW_TX_MEMPOOL_PREFIX, raw_tx_id);
        match self.db().get(&raw_tx_db_key) {
            Ok(Some(entry_bytes)) => {
                if let Ok(mut raw_tx_entry) = serde_json::from_slice::<RawTxMempoolEntry>(&entry_bytes) {
                    if raw_tx_entry.leader_id != *local_pk_hex {
                        return;
                    }
                    println!("Leader {} (Charlie) received task offer for raw_tx_id {} from leader {}.",
                        local_pk_hex, raw_tx_id, task_offer.generated_by_leader_id);

                    let mut offered_tasks_map = self.offered_validation_tasks.lock().await;
                    offered_tasks_map.entry(raw_tx_id.clone()).or_default().push(task_offer.clone());
                    println!("Stored offered task {} for raw_tx_id {}.", task_offer.task_id, raw_tx_id);

                    if offered_tasks_map.get(&raw_tx_id).map_or(false, |v| v.len() == 1) {
                        drop(offered_tasks_map);
                        self.process_and_assign_tasks_for_tx(&mut raw_tx_entry).await;
                    }

                } else { eprintln!("Error deserializing RawTxMempoolEntry for {}", raw_tx_id); }
            }
            Ok(None) => { eprintln!("RawTxMempoolEntry not found for {} when handling task offer.", raw_tx_id); }
            Err(e) => { eprintln!("DB error getting RawTxMempoolEntry for {}: {}", raw_tx_id, e); }
        }
    }

    pub async fn process_and_assign_tasks_for_tx(&mut self, raw_tx_entry: &mut RawTxMempoolEntry) {
        let local_pk_hex = &self.node_identity.public_key_hex;
        let raw_tx_id = raw_tx_entry.tx_data.calculate_hash();

        if raw_tx_entry.leader_id != *local_pk_hex { return; }

        let mut offered_tasks_map = self.offered_validation_tasks.lock().await;
        let mut tasks_to_assign_to_user = Vec::new();

        if let Some(offers) = offered_tasks_map.get_mut(&raw_tx_id) {
            for offered_task in offers.iter_mut() {
                offered_task.assigned_by_leader_id = Some(local_pk_hex.clone());
                tasks_to_assign_to_user.push(offered_task.clone());
                raw_tx_entry.validation_tasks.insert(offered_task.task_id.clone(), offered_task.clone());
            }
            offers.clear();
        }
        drop(offered_tasks_map);

        if tasks_to_assign_to_user.is_empty() { return; }

        self.tasks_assigned_to_users.lock().await.insert(raw_tx_id.clone(), tasks_to_assign_to_user.clone());

        let raw_tx_db_key = format!("{}{}", DB_RAW_TX_MEMPOOL_PREFIX, raw_tx_id);
        if let Ok(json_entry) = serde_json::to_string(raw_tx_entry) {
            if self.db().put(&raw_tx_db_key, json_entry).is_err() {
                eprintln!("Failed to update RawTxMempoolEntry {} with assigned tasks.", raw_tx_id);
            }
        }

        let assignment_message = P2PMessage::ValidationTaskAssignmentToUser {
            tasks_for_user: tasks_to_assign_to_user.clone(),
            user_public_key_hex: raw_tx_entry.tx_data.user.clone(),
            raw_tx_id: raw_tx_id.clone(),
        };
        println!("Leader {} (Charlie) would assign {} tasks to user {} for raw_tx_id {}. (Simulated send)",
            local_pk_hex, tasks_to_assign_to_user.len(), raw_tx_entry.tx_data.user, raw_tx_id
        );
    }

    pub async fn handle_user_task_completion(
        &mut self,
        task_id: String,
        raw_tx_id: String,
        user_pk_hex: String,
        completion_sig_bytes: Vec<u8>,
        completion_ts: DateTime<Utc>
    ) {
        let local_pk_hex = &self.node_identity.public_key_hex;

        let user_pub_key = match hex::decode(&user_pk_hex)
            .map_err(|e| format!("Invalid hex for user PK {}: {}", user_pk_hex,e))
            .and_then(|bytes| PublicKey::from_bytes(&bytes).map_err(|e| format!("Invalid PK bytes for {}: {}", user_pk_hex,e))) {
            Ok(pk) => pk,
            Err(e) => { eprintln!("Invalid user public key in task completion: {}", e); return; }
        };

        let message_to_verify = format!("{}{}{}", task_id, raw_tx_id, completion_ts.to_rfc3339());
        let signature = match Signature::from_bytes(&completion_sig_bytes) {
            Ok(s) => s,
            Err(_) => { eprintln!("Invalid signature format in task completion from user {}", user_pk_hex); return; }
        };

        if user_pub_key.verify(message_to_verify.as_bytes(), &signature).is_ok() {
            println!("User {}'s signature for task {} completion VERIFIED by L2 ({}).", user_pk_hex, task_id, local_pk_hex);

            let raw_tx_db_key = format!("{}{}", DB_RAW_TX_MEMPOOL_PREFIX, raw_tx_id);
            match self.db().get(&raw_tx_db_key) {
                Ok(Some(entry_bytes)) => {
                    if let Ok(raw_tx_entry) = serde_json::from_slice::<RawTxMempoolEntry>(&entry_bytes) {
                        let charlie_pk_hex = raw_tx_entry.leader_id;

                        let completed_task_info = ValidationTask {
                            task_id: task_id.clone(),
                            task_type: ValidationTaskType::UserSignatureAndBalanceValidation,
                            raw_tx_id: raw_tx_id.clone(),
                            subject_tx_id: raw_tx_id.clone(),
                            assigned_by_leader_id: None,
                            generated_by_leader_id: local_pk_hex.clone(),
                            assigned_to_user_pk_hex: Some(user_pk_hex.clone()),
                            completed: true,
                            completion_signature_bytes: Some(completion_sig_bytes),
                            completion_timestamp: Some(completion_ts),
                            completion_reported_to_origin_leader: false,
                        };

                        let forward_message = P2PMessage::ForwardUserTaskCompletionToOriginLeader {
                            completed_task: completed_task_info,
                        };
                        if let Ok(serialized_fwd_msg) = serde_json::to_vec(&forward_message) {
                             if self.gossipsub.publish(IdentTopic::new("consensus-messages"), serialized_fwd_msg).is_err() {
                                eprintln!("L2 ({}) failed to gossip ForwardUserTaskCompletionToOriginLeader for task {}", local_pk_hex, task_id);
                            }
                        }
                    } else { eprintln!("L2 ({}) error deserializing RawTxMempoolEntry for {}", local_pk_hex, raw_tx_id); }
                }
                Ok(None) => { eprintln!("L2 ({}) couldn't find RawTxMempoolEntry for {} to forward completion.", local_pk_hex, raw_tx_id); }
                Err(e) => { eprintln!("L2 ({}) DB error for RawTxMempoolEntry {}: {}", local_pk_hex, raw_tx_id, e); }
            }
        } else {
            eprintln!("User {}'s signature for task {} completion FAILED verification by L2 ({}).", user_pk_hex, task_id, local_pk_hex);
        }
    }

    pub async fn handle_forwarded_user_task_completion(&mut self, forwarded_task_completion: ValidationTask) {
        let local_pk_hex = &self.node_identity.public_key_hex;
        let raw_tx_id = forwarded_task_completion.raw_tx_id.clone();
        let task_id = forwarded_task_completion.task_id.clone();

        let raw_tx_db_key = format!("{}{}", DB_RAW_TX_MEMPOOL_PREFIX, raw_tx_id);
        match self.db().get(&raw_tx_db_key) {
            Ok(Some(entry_bytes)) => {
                if let Ok(mut raw_tx_entry) = serde_json::from_slice::<RawTxMempoolEntry>(&entry_bytes) {
                    if raw_tx_entry.leader_id != *local_pk_hex { return; }

                    if let Some(task_in_mempool) = raw_tx_entry.validation_tasks.get_mut(&task_id) {
                        if task_in_mempool.completed { return; }

                        let user_pk_hex = forwarded_task_completion.assigned_to_user_pk_hex.as_ref().unwrap();
                        let user_pub_key = match hex::decode(&user_pk_hex)
                            .map_err(|_| "Invalid hex")
                            .and_then(|bytes| PublicKey::from_bytes(&bytes).map_err(|_| "Invalid PK bytes")) {
                            Ok(pk) => pk,
                            Err(_) => { return; }
                        };

                        if forwarded_task_completion.verify_completion_signature(&user_pub_key) {
                            task_in_mempool.completed = true;
                            task_in_mempool.completion_signature_bytes = forwarded_task_completion.completion_signature_bytes.clone();
                            task_in_mempool.completion_timestamp = forwarded_task_completion.completion_timestamp;
                            task_in_mempool.completion_reported_to_origin_leader = true;

                            raw_tx_entry.validation_timestamps.push(forwarded_task_completion.completion_timestamp.unwrap());

                            if let Ok(json_entry) = serde_json::to_string(&raw_tx_entry) {
                                if self.db().put(&raw_tx_db_key, json_entry).is_err() {
                                    eprintln!("Charlie ({}) failed to save updated RawTxMempoolEntry for {}", local_pk_hex, raw_tx_id);
                                }
                            }

                            let all_user_sig_tasks_done = raw_tx_entry.validation_tasks.values()
                                .filter(|t| matches!(t.task_type, ValidationTaskType::UserSignatureAndBalanceValidation) && t.assigned_to_user_pk_hex.is_some())
                                .all(|t| t.completed);

                            if all_user_sig_tasks_done {
                                let val_task_list_key = format!("{}{}", DB_VALIDATION_TASKS_MEMPOOL_PREFIX, raw_tx_id);
                                if self.db().delete(&val_task_list_key).is_err() {
                                     eprintln!("Charlie ({}) failed to remove {} from general validation_tasks_mempool.", local_pk_hex, raw_tx_id);
                                }
                            }
                        } else { /* Verification failed */ }
                    } else { /* Task not found in mempool */ }
                } else { /* Deserialization error */ }
            }
            Ok(None) => { /* RawTX not found */ }
            Err(_) => { /* DB Error */ }
        }
    }

    // Method for Charlie to attempt to move a RawTX to ProcessingTX
    pub async fn attempt_process_raw_transaction(&mut self, raw_tx_id: &str) -> Result<(), String> {
        let local_pk_hex = self.node_identity.public_key_hex.clone();
        let db = self.db();

        // 1. Fetch the RawTxMempoolEntry
        let raw_tx_db_key = format!("{}{}", DB_RAW_TX_MEMPOOL_PREFIX, raw_tx_id);
        let raw_tx_entry_bytes = db.get(&raw_tx_db_key)
            .map_err(|e| format!("DB error fetching raw_tx {}: {}", raw_tx_id, e))?
            .ok_or_else(|| format!("RawTX {} not found for processing.", raw_tx_id))?;

        let mut raw_tx_entry: RawTxMempoolEntry = serde_json::from_slice(&raw_tx_entry_bytes)
            .map_err(|e| format!("Deserialization error for raw_tx {}: {}", raw_tx_id, e))?;

        // Ensure this node is Charlie (the originating leader)
        if raw_tx_entry.leader_id != local_pk_hex {
            return Ok(()); // Not this node's responsibility to process this one
        }

        // 2. Check completion criteria
        //  a. All assigned 'UserSignatureAndBalanceValidation' tasks are complete
        let all_user_tasks_complete = raw_tx_entry.validation_tasks.values()
            .filter(|task| matches!(task.task_type, ValidationTaskType::UserSignatureAndBalanceValidation) &&
                           task.assigned_to_user_pk_hex.is_some())
            .all(|task| task.completed);

        if !all_user_tasks_complete {
            // println!("RawTX {}: Not all user tasks complete. Cannot process yet.", raw_tx_id);
            return Ok(());
        }

        //  b. Sufficient validation timestamps
        if raw_tx_entry.validation_timestamps.len() < MIN_VALIDATION_TIMESTAMPS_FOR_PROCESSING {
            // println!("RawTX {}: Insufficient validation timestamps ({} < {}). Cannot process yet.",
            //     raw_tx_id, raw_tx_entry.validation_timestamps.len(), MIN_VALIDATION_TIMESTAMPS_FOR_PROCESSING);
            return Ok(());
        }

        println!("RawTX {}: All criteria met. Processing into ProcessingTxMempoolEntry.", raw_tx_id);

        // 3. Average validation_timestamps
        if raw_tx_entry.validation_timestamps.is_empty() {
            return Err(format!("No validation timestamps for raw_tx {} despite passing checks.", raw_tx_id));
        }
        let sum_timestamps_nanos: i64 = raw_tx_entry.validation_timestamps.iter().map(|dt| dt.timestamp_nanos()).sum();
        let avg_timestamp_nanos = sum_timestamps_nanos / raw_tx_entry.validation_timestamps.len() as i64;
        let averaged_validation_timestamp = DateTime::<Utc>::from_utc(
            chrono::NaiveDateTime::from_timestamp(
                avg_timestamp_nanos / 1_000_000_000,
                (avg_timestamp_nanos % 1_000_000_000) as u32
            ), Utc);

        println!("RawTX {}: Averaged timestamp: {}", raw_tx_id, averaged_validation_timestamp);

        // 4. Create ProcessingTxMempoolEntry
        let tx_data_hash = raw_tx_entry.tx_data.calculate_hash();
        let processing_tx_id_material = format!("{}{}", averaged_validation_timestamp.to_rfc3339(), tx_data_hash);
        let mut hasher = Sha256::new();
        hasher.update(processing_tx_id_material.as_bytes());
        let processing_tx_id = format!("proctx_{:x}", hasher.finalize());

        let mut processing_entry = ProcessingTxMempoolEntry {
            tx_data: raw_tx_entry.tx_data.clone(),
            averaged_validation_timestamp,
            leader_signature_bytes: vec![],
            leader_id: local_pk_hex.clone(),
            tx_id: processing_tx_id,
        };

        if let Some(keypair) = &self.node_identity.keypair {
            processing_entry = processing_entry.sign(keypair);
        } else {
            return Err(format!("Leader {} has no keypair to sign ProcessingTxMempoolEntry for {}", local_pk_hex, raw_tx_id));
        }

        // 5. Atomically: remove from raw_tx_mempool, store in processing_tx_mempool
        let processing_tx_db_key = format!("{}{}", DB_PROCESSING_TX_MEMPOOL_PREFIX, processing_entry.tx_id);
        let processing_json_entry = serde_json::to_string(&processing_entry)
            .map_err(|e| format!("Serialization error for processing_tx {}: {}", processing_entry.tx_id, e))?;

        let mut batch = WriteBatch::default();
        batch.delete(&raw_tx_db_key);
        batch.put(&processing_tx_db_key, processing_json_entry);

        db.write(batch).map_err(|e| format!("DB error finalizing processing for raw_tx {}: {}", raw_tx_id, e))?;
        println!("RawTX {} processed. New ProcessingTX ID: {}. Stored.", raw_tx_id, processing_entry.tx_id);

        // 6. Add new validation task for validators to general validation_tasks_mempool
        let validator_task_key_in_pool = format!("{}{}", DB_VALIDATION_TASKS_MEMPOOL_PREFIX, processing_entry.tx_id);
        let validator_task_details = ValidationTask::new(
            ValidationTaskType::LeaderTimestampMathCheck,
            raw_tx_id.to_string(),
            processing_entry.tx_id.clone(),
            local_pk_hex.clone(),
            None
        );
        let validator_task_json = serde_json::to_string(&validator_task_details)
            .map_err(|e| format!("Serialization error for validator task for {}: {}", processing_entry.tx_id, e))?;

        db.put(&validator_task_key_in_pool, validator_task_json)
            .map_err(|e| format!("DB error adding validator task for {}: {}", processing_entry.tx_id, e))?;
        println!("Added new validation task for validators for ProcessingTX ID: {}", processing_entry.tx_id);

        let val_task_list_key = format!("{}{}", DB_VALIDATION_TASKS_MEMPOOL_PREFIX, raw_tx_id);
        if db.delete(&val_task_list_key).is_err() {
            eprintln!("Warning: Failed to remove {} from general validation_tasks_mempool after processing.", raw_tx_id);
        } else {
            println!("Removed {} from general validation_tasks_mempool as it's now processed.", raw_tx_id);
        }
        Ok(())
    }

    // --- Transaction Workflow Step 6 ---

    // Simulate a validator picking up and completing a LeaderTimestampMathCheck task
    pub async fn simulate_validator_completing_math_check(&mut self, processing_tx_id: &str) -> Result<(), String> {
        let local_pk_hex = self.node_identity.public_key_hex.clone();
        let db = self.db();

        // 1. Fetch the ProcessingTxMempoolEntry (that Charlie created)
        let proctx_db_key = format!("{}{}", DB_PROCESSING_TX_MEMPOOL_PREFIX, processing_tx_id);
        let proctx_bytes = db.get(&proctx_db_key)
            .map_err(|e| format!("DB error fetching proctx {}: {}", processing_tx_id, e))?
            .ok_or_else(|| format!("ProcessingTX {} not found for validator math check.", processing_tx_id))?;

        let processing_entry: ProcessingTxMempoolEntry = serde_json::from_slice(&proctx_bytes)
            .map_err(|e| format!("Deserialization error for proctx {}: {}", processing_tx_id, e))?;

        // 2. Perform validation (as a validator)
        //  a. Verify Charlie's signature on ProcessingTxMempoolEntry
        let charlie_pub_key = match hex::decode(&processing_entry.leader_id)
            .map_err(|_| "Invalid hex for Charlie's PK")
            .and_then(|bytes| PublicKey::from_bytes(&bytes).map_err(|_| "Invalid PK bytes for Charlie")) {
            Ok(pk) => pk,
            Err(e) => return Err(format!("Invalid leader_id PK in proctx {}: {}", processing_tx_id, e)),
        };
        if !processing_entry.verify_leader_signature(&charlie_pub_key) {
            return Err(format!("Validator: Charlie's signature verification failed for proctx {}", processing_tx_id));
        }

        //  b. Re-calculate tx_id (hash of {averaged_timestamp + tx_data_hash})
        let tx_data_hash = processing_entry.tx_data.calculate_hash();
        let expected_tx_id_material = format!("{}{}", processing_entry.averaged_validation_timestamp.to_rfc3339(), tx_data_hash);
        let mut hasher = Sha256::new();
        hasher.update(expected_tx_id_material.as_bytes());
        let calculated_tx_id = format!("proctx_{:x}", hasher.finalize());

        if calculated_tx_id != processing_entry.tx_id {
            return Err(format!("Validator: tx_id mismatch for proctx {}. Expected {}, got {}",
                processing_tx_id, calculated_tx_id, processing_entry.tx_id));
        }
        println!("Validator ({}): Math check and signature for proctx {} PASSED.", local_pk_hex, processing_tx_id);

        // 3. Validator signs the processing_entry.tx_id to attest completion
        let validator_signature_on_tx_id = self.node_identity.keypair.as_ref()
            .ok_or_else(|| "Validator keypair not found".to_string())?
            .sign(processing_entry.tx_id.as_bytes()).to_bytes().to_vec();

        // 4. Broadcast VerifiedProcessingTxBroadcast to N random leaders
        let leaders_lock = self.current_leaders.lock().await;
        if leaders_lock.is_empty() {
            return Err("No leaders available to broadcast verified processing TX.".to_string());
        }
        // Simple random selection (not cryptographically secure, but ok for simulation)
        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();
        let chosen_leaders = leaders_lock.as_slice()
            .choose_multiple(&mut rng, NUM_LEADERS_FOR_VALIDATOR_BROADCAST.min(leaders_lock.len()))
            .cloned()
            .collect::<Vec<String>>();
        drop(leaders_lock);

        let broadcast_message = P2PMessage::VerifiedProcessingTxBroadcast {
            processing_entry: processing_entry.clone(),
            validator_id_pk_hex: local_pk_hex.clone(),
            validator_signature_on_tx_id,
        };

        // This should be a targeted send to `chosen_leaders` PeerIds.
        // For now, using gossipsub and relying on leaders to pick it up.
        // A real implementation would resolve chosen_leaders (PK hex) to PeerIds.
        if let Ok(serialized_broadcast) = serde_json::to_vec(&broadcast_message) {
            if self.gossipsub.publish(IdentTopic::new("consensus-messages"), serialized_broadcast).is_err() {
                 eprintln!("Validator ({}): Failed to gossip VerifiedProcessingTxBroadcast for {}", local_pk_hex, processing_tx_id);
            } else {
                println!("Validator ({}): Gossiped VerifiedProcessingTxBroadcast for {} to (conceptually) {:?}.",
                    local_pk_hex, processing_tx_id, chosen_leaders);
            }
        }
        Ok(())
    }

    // Leader handles a VerifiedProcessingTxBroadcast from a validator
    pub async fn handle_verified_processing_tx_broadcast(
        &mut self,
        verified_entry: ProcessingTxMempoolEntry,
        validator_id: String,
        _validator_sig: Vec<u8> // TODO: Verify this signature
    ) -> Result<(), String> {
        let local_pk_hex = self.node_identity.public_key_hex.clone();
        let db = self.db();
        let proctx_id = verified_entry.tx_id.clone();
        let raw_tx_id = verified_entry.tx_data.calculate_hash(); // Original raw_tx_id

        println!("Leader ({}): Received VerifiedProcessingTxBroadcast for proctx {} from validator {}.",
            local_pk_hex, proctx_id, validator_id);

        // TODO: Verify validator_sig against validator_id's public key and proctx_id

        // 1. Store/Update in local processing_tx_mempool
        let proctx_db_key = format!("{}{}", DB_PROCESSING_TX_MEMPOOL_PREFIX, proctx_id);
        if db.get(&proctx_db_key).map_err(|e|e.to_string())?.is_none() {
            let proctx_json = serde_json::to_string(&verified_entry)
                .map_err(|e| format!("Serialization error for proctx {}: {}", proctx_id, e))?;
            db.put(&proctx_db_key, proctx_json)
                .map_err(|e| format!("DB error storing proctx {}: {}", proctx_id, e))?;
            println!("Leader ({}): Stored proctx {} from validator broadcast.", local_pk_hex, proctx_id);
        }

        // 2. Perform DLT-specific finality task (digital root)
        let digital_root = calculate_digital_root_of_hex_string(&proctx_id);
        println!("Leader ({}): Calculated digital root {} for proctx {}.", local_pk_hex, digital_root, proctx_id);

        // 3. Store in final_tx_mempool (tx_mempool in README)
        // Value could be just the digital root, or a structure containing it and other relevant info.
        // For now, store { "tx_id": proctx_id, "digital_root": digital_root, "original_tx_data": verified_entry.tx_data }
        #[derive(Serialize, Deserialize)]
        struct FinalTxEntry {
            tx_id: String,
            digital_root: u32,
            original_tx_data: TxData,
            processed_timestamp: DateTime<Utc>,
        }
        let final_entry = FinalTxEntry {
            tx_id: proctx_id.clone(),
            digital_root,
            original_tx_data: verified_entry.tx_data.clone(),
            processed_timestamp: Utc::now(),
        };
        let final_tx_db_key = format!("{}{}", DB_FINAL_TX_MEMPOOL_PREFIX, proctx_id);
        let final_json_entry = serde_json::to_string(&final_entry)
            .map_err(|e| format!("Serialization error for final_tx {}: {}", proctx_id, e))?;
        db.put(&final_tx_db_key, final_json_entry)
            .map_err(|e| format!("DB error storing final_tx {}: {}", proctx_id, e))?;
        println!("Leader ({}): Stored proctx {} with digital root in final_tx_mempool.", local_pk_hex, proctx_id);

        // 4. Cleanup raw_tx_mempool and associated validation_tasks_mempool entries
        // This leader might have the original raw_tx if it was Charlie or received early gossip.
        let raw_tx_db_key = format!("{}{}", DB_RAW_TX_MEMPOOL_PREFIX, raw_tx_id);
        let val_task_for_raw_tx_key = format!("{}{}", DB_VALIDATION_TASKS_MEMPOOL_PREFIX, raw_tx_id); // General task list for raw_tx
        let val_task_for_proctx_key = format!("{}{}", DB_VALIDATION_TASKS_MEMPOOL_PREFIX, proctx_id); // Validator task for proctx

        let mut batch = WriteBatch::default();
        batch.delete(&raw_tx_db_key);
        batch.delete(&val_task_for_raw_tx_key);
        batch.delete(&val_task_for_proctx_key); // Validator task now conceptually "done" by this flow

        db.write(batch).map_err(|e| format!("DB error cleaning up for raw_tx {}: {}", raw_tx_id, e))?;
        println!("Leader ({}): Cleaned up raw_tx data for original raw_tx_id {} after processing proctx {}.",
            local_pk_hex, raw_tx_id, proctx_id);

        // 5. Gossip the ProcessingTxMempoolEntry to all other leaders
        let gossip_message = P2PMessage::ProcessingTransactionGossip(Box::new(verified_entry));
        if let Ok(serialized_gossip) = serde_json::to_vec(&gossip_message) {
            if self.gossipsub.publish(IdentTopic::new("consensus-messages"), serialized_gossip).is_err() {
                eprintln!("Leader ({}): Failed to gossip ProcessingTransactionGossip for {}", local_pk_hex, proctx_id);
            } else {
                println!("Leader ({}): Gossiped ProcessingTransactionGossip for {}.", local_pk_hex, proctx_id);
            }
        }
        Ok(())
    }

    // Handle gossiped ProcessingTxMempoolEntry (from other leaders)
    pub async fn handle_processing_transaction_gossip(&mut self, entry: ProcessingTxMempoolEntry) -> Result<(), String> {
        let local_pk_hex = self.node_identity.public_key_hex.clone();
        let db = self.db();
        let proctx_id = entry.tx_id.clone();
        let raw_tx_id = entry.tx_data.calculate_hash();

        println!("Leader ({}): Received ProcessingTransactionGossip for proctx {}.", local_pk_hex, proctx_id);

        // 1. Verify Charlie's signature (leader_id on the entry)
        let charlie_pub_key = match hex::decode(&entry.leader_id)
            .map_err(|_| "Invalid hex for Charlie's PK on gossiped proctx")
            .and_then(|bytes| PublicKey::from_bytes(&bytes).map_err(|_| "Invalid PK bytes for Charlie on gossiped proctx")) {
            Ok(pk) => pk,
            Err(e) => return Err(format!("Invalid leader_id PK in gossiped proctx {}: {}", proctx_id, e)),
        };
        if !entry.verify_leader_signature(&charlie_pub_key) {
            return Err(format!("Charlie's signature verification failed for gossiped proctx {}", proctx_id));
        }

        // 2. Store/Update in local processing_tx_mempool (if new)
        let proctx_db_key = format!("{}{}", DB_PROCESSING_TX_MEMPOOL_PREFIX, proctx_id);
        if db.get(&proctx_db_key).map_err(|e|e.to_string())?.is_none() {
             let proctx_json = serde_json::to_string(&entry)
                .map_err(|e| format!("Serialization error for gossiped proctx {}: {}", proctx_id, e))?;
            db.put(&proctx_db_key, proctx_json)
                .map_err(|e| format!("DB error storing gossiped proctx {}: {}", proctx_id, e))?;
        }

        // 3. Perform DLT-specific finality task (digital root)
        let digital_root = calculate_digital_root_of_hex_string(&proctx_id);

        // 4. Store in final_tx_mempool (if new)
        let final_tx_db_key = format!("{}{}", DB_FINAL_TX_MEMPOOL_PREFIX, proctx_id);
        if db.get(&final_tx_db_key).map_err(|e|e.to_string())?.is_none() {
            #[derive(Serialize, Deserialize)]
            struct FinalTxEntry { tx_id: String, digital_root: u32, original_tx_data: TxData, processed_timestamp: DateTime<Utc> }
            let final_entry = FinalTxEntry {
                tx_id: proctx_id.clone(), digital_root, original_tx_data: entry.tx_data.clone(), processed_timestamp: Utc::now(),
            };
            let final_json_entry = serde_json::to_string(&final_entry)
                .map_err(|e| format!("Serialization error for final_tx (from gossip) {}: {}", proctx_id, e))?;
            db.put(&final_tx_db_key, final_json_entry)
                .map_err(|e| format!("DB error storing final_tx (from gossip) {}: {}", proctx_id, e))?;
             println!("Leader ({}): Stored gossiped proctx {} with digital root in final_tx_mempool.", local_pk_hex, proctx_id);
        }

        // 5. Cleanup associated raw TX data
        let raw_tx_db_key = format!("{}{}", DB_RAW_TX_MEMPOOL_PREFIX, raw_tx_id);
        let val_task_for_raw_tx_key = format!("{}{}", DB_VALIDATION_TASKS_MEMPOOL_PREFIX, raw_tx_id);
        let val_task_for_proctx_key = format!("{}{}", DB_VALIDATION_TASKS_MEMPOOL_PREFIX, proctx_id);
        let mut batch = WriteBatch::default();
        batch.delete(&raw_tx_db_key);
        batch.delete(&val_task_for_raw_tx_key);
        batch.delete(&val_task_for_proctx_key);
        db.write(batch).map_err(|e| format!("DB error cleaning up (from gossip) for raw_tx {}: {}", raw_tx_id, e))?;

        Ok(())
    }

    // --- Invalidation Handling ---
    async fn cleanup_transaction_data(&mut self, raw_tx_id_to_clean: &str, proctx_id_to_clean: Option<&str>) {
        let db = self.db();
        println!("Cleaning up data for raw_tx_id: {}, proctx_id: {:?}", raw_tx_id_to_clean, proctx_id_to_clean);

        let mut batch = WriteBatch::default();

        let raw_tx_db_key = format!("{}{}", DB_RAW_TX_MEMPOOL_PREFIX, raw_tx_id_to_clean);
        batch.delete(&raw_tx_db_key);

        let val_task_raw_key = format!("{}{}", DB_VALIDATION_TASKS_MEMPOOL_PREFIX, raw_tx_id_to_clean);
        batch.delete(&val_task_raw_key);

        let mut utxos_to_unlock = Vec::new();
        let iter = db.prefix_iterator(DB_LOCKED_UTXO_MEMPOOL_PREFIX.as_bytes());
        for item in iter {
            if let Ok((utxo_key_bytes, locked_by_raw_tx_id_bytes)) = item {
                let locked_by_raw_tx_id = String::from_utf8_lossy(&locked_by_raw_tx_id_bytes);
                if locked_by_raw_tx_id == raw_tx_id_to_clean {
                    utxos_to_unlock.push(utxo_key_bytes.to_vec());
                }
            }
        }
        for utxo_key in utxos_to_unlock {
            batch.delete(&utxo_key);
        }

        if let Some(pid) = proctx_id_to_clean {
            let proctx_db_key = format!("{}{}", DB_PROCESSING_TX_MEMPOOL_PREFIX, pid);
            batch.delete(&proctx_db_key);

            let val_task_proctx_key = format!("{}{}", DB_VALIDATION_TASKS_MEMPOOL_PREFIX, pid);
            batch.delete(&val_task_proctx_key);

            let final_tx_db_key = format!("{}{}", DB_FINAL_TX_MEMPOOL_PREFIX, pid);
            batch.delete(&final_tx_db_key);
        }

        if let Err(e) = db.write(batch) {
            eprintln!("Error during batch deletion for invalidated tx {}: {}", raw_tx_id_to_clean, e);
        } else {
            println!("Successfully cleaned up DB entries for invalidated tx {}.", raw_tx_id_to_clean);
        }

        self.offered_validation_tasks.lock().await.remove(raw_tx_id_to_clean);
        self.tasks_assigned_to_users.lock().await.remove(raw_tx_id_to_clean);
        // TODO: Clear other in-memory caches/maps if any.
    }

    pub async fn handle_transaction_invalidation_notice(&mut self, tx_id_to_invalidate: &str, reason: &str) {
        println!("Received TransactionInvalidationNotice for tx_id: {} (can be raw or proctx), Reason: {}", tx_id_to_invalidate, reason);

        // Determine if tx_id_to_invalidate is raw or processed to call cleanup appropriately.
        // This is a simplification: we might need more context or check both forms.
        // If it starts with "proctx_", assume it's a processed_tx_id.
        // Otherwise, assume it's a raw_tx_id.

        let mut raw_id_to_use = tx_id_to_invalidate.to_string();
        let mut proctx_id_maybe: Option<String> = None;

        if tx_id_to_invalidate.starts_with("proctx_") {
            proctx_id_maybe = Some(tx_id_to_invalidate.to_string());
            // We need to find the original raw_tx_id. This requires fetching the proctx entry.
            let db = self.db();
            let proctx_db_key = format!("{}{}", DB_PROCESSING_TX_MEMPOOL_PREFIX, tx_id_to_invalidate);
            if let Ok(Some(bytes)) = db.get(proctx_db_key) {
                if let Ok(entry) = serde_json::from_slice::<ProcessingTxMempoolEntry>(&bytes) {
                    raw_id_to_use = entry.tx_data.calculate_hash();
                } else {
                    eprintln!("Could not deserialize ProcessingTxMempoolEntry for {} to find raw_tx_id during invalidation.", tx_id_to_invalidate);
                    // Cannot proceed reliably without raw_tx_id for full cleanup
                    return;
                }
            } else {
                 eprintln!("ProcessingTxMempoolEntry {} not found during invalidation to get raw_tx_id.", tx_id_to_invalidate);
                 // If only proctx_id is given and not found, we might not be able to find original raw_tx_id.
                 // However, cleanup_transaction_data can still try to clean based on proctx_id alone for some parts.
            }
        }

        self.cleanup_transaction_data(&raw_id_to_use, proctx_id_maybe.as_deref()).await;

        // Re-gossip the invalidation notice
        // Avoid re-gossiping if this node is the one that just processed it (e.g. to prevent loops if not handled carefully)
        // For now, simple re-gossip.
        let notice_message = P2PMessage::TransactionInvalidationNotice {
            tx_id: tx_id_to_invalidate.to_string(), // Use the ID as received in the notice
            reason: reason.to_string(),
        };
        if let Ok(serialized_notice) = serde_json::to_vec(&notice_message) {
            if self.gossipsub.publish(IdentTopic::new("consensus-messages"), serialized_notice).is_err() {
                eprintln!("Failed to re-gossip TransactionInvalidationNotice for {}", tx_id_to_invalidate);
            }
        }
    }

    // Example of where an invalidation might be triggered:
    // (This is a simplified example; actual triggers would be in specific validation methods)
    pub async fn trigger_invalidation_if_condition_fails(&mut self, raw_tx_id: &str, reason: String) {
        println!("Condition failed for raw_tx_id: {}. Triggering invalidation. Reason: {}", raw_tx_id, reason);
        self.cleanup_transaction_data(raw_tx_id, None).await; // Assuming no proctx_id known at this point of failure

        let notice_message = P2PMessage::TransactionInvalidationNotice {
            tx_id: raw_tx_id.to_string(),
            reason,
        };
        if let Ok(serialized_notice) = serde_json::to_vec(&notice_message) {
            if self.gossipsub.publish(IdentTopic::new("consensus-messages"), serialized_notice).is_err() {
                eprintln!("Failed to gossip initial TransactionInvalidationNotice for {}", raw_tx_id);
            }
        }
    }

}


// Event processing for mDNS
impl NetworkBehaviourEventProcess<MdnsEvent> for ConsensusBehaviour {
    fn inject_event(&mut self, event: MdnsEvent) {
        if let MdnsEvent::Discovered(list) = event {
            for (peer_id, _multiaddr) in list {
                println!("mDNS discovered a new peer: {}", peer_id);
                self.gossipsub.add_explicit_peer(&peer_id);
            }
        }
    }
}

// Event processing for Gossipsub
impl NetworkBehaviourEventProcess<GossipsubEvent> for ConsensusBehaviour {
    fn inject_event(&mut self, event: GossipsubEvent) {
        match event {
            GossipsubEvent::Message {
                propagation_source:เผยแพร่,
                message_id: _id,
                message,
            } => {
                let msg_str = String::from_utf8_lossy(&message.data);
                 match serde_json::from_slice::<P2PMessage>(&message.data) {
                    Ok(p2p_message) => {
                        let mut gossiped_tx_sender_clone = self.gossiped_tx_sender.clone();

                        let self_clone_node_identity = Arc::clone(&self.node_identity);
                        let self_clone_received_uptime_data = Arc::clone(&self.received_uptime_data);
                        let self_clone_received_nominations = Arc::clone(&self.received_nominations);
                        let self_clone_votes_for_round = Arc::clone(&self.votes_for_round);
                        let self_clone_current_leaders = Arc::clone(&self.current_leaders);
                        let self_clone_last_leader_list_hash = Arc::clone(&self.last_leader_list_hash);
                        let self_clone_election_in_progress = Arc::clone(&self.election_in_progress);
                        let source_peer_id_option = message.source;

                        // Clone senders for the async block
                        let offer_sender = self.offer_val_task_sender.clone();
                        let user_completion_sender = self.user_task_completion_sender.clone();
                        let forwarded_completion_sender = self.forwarded_completion_sender.clone();
                        let gossiped_tx_sender_clone_for_match = self.gossiped_tx_sender.clone();
                let verified_proctx_sender_clone = self.verified_processing_tx_sender.clone();
                // Assuming a new channel for invalidation notices if needed for async processing,
                // or handle directly if simple enough (like re-gossip).
                // For now, let's add a channel for it.
                let invalidation_notice_sender_clone = self.invalidation_notice_sender.clone();
                 let client_tx_sender_clone = self.client_submitted_tx_sender.clone();


                        tokio::spawn(async move {
                            match p2p_message {
                                P2PMessage::RawTransactionGossip(entry) => {
                                    if let Err(e) = gossiped_tx_sender_clone_for_match.send(*entry).await {
                                         eprintln!("Error sending RawTransactionGossip to channel: {}", e);
                                    }
                                }
                                P2PMessage::OfferValidationTaskToOriginLeader { .. } => {
                                    if let Err(e) = offer_sender.send(p2p_message).await {
                                        eprintln!("Error sending OfferValidationTaskToOriginLeader to channel: {}", e);
                                    }
                                }
                                P2PMessage::UserValidationTaskCompletion { .. } => {
                                     if let Err(e) = user_completion_sender.send(p2p_message).await {
                                        eprintln!("Error sending UserValidationTaskCompletion to channel: {}", e);
                                    }
                                }
                                P2PMessage::ForwardUserTaskCompletionToOriginLeader { .. } => {
                                     if let Err(e) = forwarded_completion_sender.send(p2p_message).await {
                                        eprintln!("Error sending ForwardUserTaskCompletionToOriginLeader to channel: {}", e);
                                    }
                                }
                                P2PMessage::VerifiedProcessingTxBroadcast{ .. } => {
                                     if let Err(e) = verified_proctx_sender_clone.send(p2p_message).await {
                                        eprintln!("Error sending VerifiedProcessingTxBroadcast to channel: {}", e);
                                    }
                                }
                                P2PMessage::TransactionInvalidationNotice{ .. } => {
                                    if let Err(e) = invalidation_notice_sender_clone.send(p2p_message).await {
                                        eprintln!("Error sending TransactionInvalidationNotice to channel: {}", e);
                                    }
                                }
                                P2PMessage::ClientSubmitRawTransaction(tx_data) => {
                                    // This message is TxData, not P2PMessage enum
                                    if let Err(e) = client_tx_sender_clone.send(tx_data).await {
                                        eprintln!("Error sending ClientSubmitRawTransaction to channel: {}", e);
                                    }
                                }
                                // Explicitly list other existing handlers or use a wildcard
                                P2PMessage::Pulse => { /* Placeholder for actual Pulse handling if done in async block */ }
                                P2PMessage::PulseResponse { .. } => { /* Placeholder */ }
                                P2PMessage::UptimeDataBroadcast(_) => { /* Placeholder */ }
                                P2PMessage::LeaderNominations { .. } => { /* Placeholder */ }
                                P2PMessage::LeaderElectionVoteMsg(_) => { /* Placeholder */ }
                                P2PMessage::NewLeaderList { .. } => { /* Placeholder */ }
                                P2PMessage::ValidationTaskAssignmentToUser{..} => {/* This message is not expected from peers */}
                                P2PMessage::ProcessingTransactionGossip(entry) => {
                                    // This can be handled directly or also via channel if complex
                                    // For now, direct handling in this async block is complex due to &mut self.
                                    // It's better to send it to a channel for main loop processing.
                                    // This needs a new channel similar to VerifiedProcessingTxBroadcast.
                                    // For now, this specific gossip is not being put on a new channel.
                                    // This means handle_processing_transaction_gossip needs to be callable from here
                                    // or the main loop needs another channel for it.
                                     eprintln!("Received ProcessingTransactionGossip directly in spawn, needs channel for proper handling.");
                                }
                            }
                        });
                    }
                    Err(e) => {
                        eprintln!("Failed to deserialize P2PMessage: {:?}, raw data: {}", e, msg_str);
                    }
                }
            }
            _ => {}
        }
    }
}


pub async fn start_node(node_identity: NodeIdentity, db_path_str: &str) -> Result<(), Box<dyn std::error::Error>> {
    let app_node_identity = Arc::new(node_identity);

    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());
    println!("Local libp2p peer id: {}", local_peer_id);
    println!("Node application identity public key: {}", app_node_identity.public_key_hex);

    let transport = TokioTcpTransport::new(TcpConfig::default().nodelay(true))
        .upgrade(upgrade::Version::V1)
        .authenticate(noise::NoiseAuthenticated::xx(&local_key)?)
        .multiplex(yamux::YamuxConfig::default())
        .timeout(std::time::Duration::from_secs(20))
        .boxed();

    let gossipsub_config = gossipsub::GossipsubConfigBuilder::default()
        .heartbeat_interval(Duration::from_secs(10))
        .validation_mode(ValidationMode::Strict)
        .message_id_fn(|message: &GossipsubMessage| {
            let mut s = DefaultHasher::new();
            message.data.hash(&mut s);
            gossipsub::MessageId::from(s.finish().to_string())
        })
        .build()?;

    let node_signing_key = app_node_identity.keypair.as_ref().ok_or("Node keypair not available for signing")?.clone();
    let mut gossipsub: gossipsub::Gossipsub =
        gossipsub::Gossipsub::new(MessageAuthenticity::Signed(node_signing_key), gossipsub_config)?;
    let topic = IdentTopic::new("consensus-messages");
    gossipsub.subscribe(&topic)?;

    let mut db_opts = Options::default();
    db_opts.create_if_missing(true);
    let db = Arc::new(DB::open(&db_opts, db_path_str).expect("Failed to open RocksDB"));

    // Create channel for gossiped transactions
    let (gossiped_tx_sender, gossiped_tx_receiver) = mpsc::channel::<RawTxMempoolEntry>(100);
    let (offer_val_task_sender, offer_val_task_receiver) = mpsc::channel(100);
    let (user_task_completion_sender, user_task_completion_receiver) = mpsc::channel(100);
    let (forwarded_completion_sender, forwarded_completion_receiver) = mpsc::channel(100);
    let (simulate_alice_completion_sender, simulate_alice_completion_receiver) = mpsc::channel(10);
    let (verified_processing_tx_sender, verified_processing_tx_receiver) = mpsc::channel(100);
    let (invalidation_notice_sender, invalidation_notice_receiver) = mpsc::channel(100); // For Invalidation
    let (client_submitted_tx_sender, client_submitted_tx_receiver) = mpsc::channel(100); // For client TXs


    let mut swarm = {
        let mdns_config = MdnsConfig {
            ttl: Duration::from_secs(60), query_interval: Duration::from_secs(5), ..Default::default()
        };
        let mdns = Mdns::new(mdns_config).await?;

        let behaviour = ConsensusBehaviour {
            gossipsub, mdns, db, local_peer_id,
            node_identity: Arc::clone(&app_node_identity),
            current_leaders: Arc::new(Mutex::new(Vec::new())),
            last_leader_list_hash: Arc::new(Mutex::new(None)),
            received_uptime_data: Arc::new(Mutex::new(HashMap::new())),
            received_nominations: Arc::new(Mutex::new(HashMap::new())),
            election_round: Arc::new(Mutex::new(0)),
            votes_for_round: Arc::new(Mutex::new(HashMap::new())),
            election_in_progress: Arc::new(Mutex::new(false)),
            last_uptime_broadcast_time: Arc::new(Mutex::new(None)),
            election_phase_start_time: Arc::new(Mutex::new(None)),
            gossiped_tx_sender,
            gossiped_tx_receiver,
            offered_validation_tasks: Arc::new(Mutex::new(HashMap::new())),
            tasks_assigned_to_users: Arc::new(Mutex::new(HashMap::new())),
            offer_val_task_receiver,
            offer_val_task_sender,
            user_task_completion_receiver,
            user_task_completion_sender,
            forwarded_completion_receiver,
            forwarded_completion_sender,
            simulate_alice_completion_sender,
            simulate_alice_completion_receiver,
            verified_processing_tx_sender, // For Step 6
            verified_processing_tx_receiver,
            invalidation_notice_sender, // For Invalidation
            invalidation_notice_receiver, // For Invalidation
            client_submitted_tx_sender, // For client TXs
            client_submitted_tx_receiver, // For client TXs
        };
        SwarmBuilder::new(transport, behaviour, local_peer_id.clone())
            .executor(Box::new(|fut| { tokio::spawn(fut); }))
            .build()
    };

    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    let mut pulse_interval = interval(Duration::from_secs(20));
    let mut prune_interval = interval(Duration::from_secs(60));
    let mut leader_election_interval = interval(Duration::from_secs(UPTIME_BROADCAST_INTERVAL_SECS));
    // Rename election_logic_tick to a more general periodic_processing_tick
    let mut periodic_processing_tick = interval(Duration::from_secs(10)); // Can adjust timing

    // Test: Simulate receiving a transaction after a delay (e.g., from a client)
    // This should only run if the node considers itself a leader.
    // For testing, we can bypass the leader check or assume it's a leader.
    let test_tx_node_identity = Arc::clone(&app_node_identity);
    let test_tx_current_leaders = Arc::clone(&swarm.behaviour().current_leaders); // Assuming direct access for test
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(30)).await; // Wait for network to settle

        let mut is_leader_for_test = false;
        { // Scope for leader lock
            let leaders = test_tx_current_leaders.lock().await;
            if leaders.is_empty() { // If no leaders elected yet, assume this node can act as one for test
                is_leader_for_test = true;
                 println!("TEST TX: No leaders elected, node {} will attempt to process a test tx.", test_tx_node_identity.public_key_hex);
            } else if leaders.contains(&test_tx_node_identity.public_key_hex) {
                is_leader_for_test = true;
            }
        }


        if is_leader_for_test {
            println!("TEST TX: Node {} is acting as leader, creating dummy transaction.", test_tx_node_identity.public_key_hex);
            let (tx, _keypair) = TxData::new_dummy_signed(
                test_tx_node_identity.public_key_hex.clone(), // Alice (this node)
                "bob_dummy_pubkey_hex".to_string(),          // Bob
                100,                                         // Amount to Bob
                200,                                         // UTXO from Alice
                10.0,                                        // Stake
                1.0                                          // Fee
            );
            // This is where the node would call its own processing logic.
            // This requires getting `&mut swarm` which is not available in this spawned task.
            // This highlights that `handle_incoming_raw_transaction` should probably be callable
            // via a message to the main swarm loop, or the swarm/behaviour itself needs to expose
            // an MPSC sender to which such "client" requests can be sent.

            // For now, this test code cannot directly call `swarm.behaviour_mut().handle_incoming_raw_transaction`.
            // This is a structural point for how external requests (like new TXs) are fed into the node.
            println!("TEST TX: Dummy transaction created: {:?}. Manual call to handler needed in main loop.", tx.calculate_hash());
            // TODO: For actual test, send this `tx` to the node itself via a client request channel.
            // For now, we can manually construct a task completion later if we know task_ids.
        } else {
            println!("TEST TX: Node {} is NOT a leader, not creating dummy transaction.", test_tx_node_identity.public_key_hex);
        }
    });

    // Test: Simulate Alice completing a task after some time.
    // This requires knowing a task_id generated by some L2 for a tx by Alice (this node).
    // This is tricky to automate perfectly without full client-server interaction for task polling.
    // We'll use the `simulate_alice_completion_sender` for a very basic simulation.
    let alice_sim_sender = swarm.behaviour().simulate_alice_completion_sender.clone();
    let alice_sim_node_id = Arc::clone(&app_node_identity); // Alice is this node in the test
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(60)).await; // Wait longer, after tasks might be offered/assigned

        // This node (Alice) needs to find a task assigned to it.
        // This would normally come from its client polling Charlie.
        // Here, we check Charlie's (this node's) `tasks_assigned_to_users` for a task for itself.
        // This is highly artificial for testing.

        // We need a way to get a task_id that this node (as Alice) would complete,
        // and the raw_tx_id it belongs to.
        // This part is difficult to simulate realistically without a proper client interaction model.
        // For now, we are demonstrating the channel, but a meaningful completion is hard to craft here.
        // Let's assume a task_id "simulated_task_for_alice" and raw_tx_id "simulated_raw_tx_for_alice"
        // would be known if the test TX processing path was complete.

        // A better test would be:
        // 1. Node A (Alice) creates tx.
        // 2. Node B (Charlie, leader) processes it. RawTxMempoolEntry created.
        // 3. Node C (L2, leader) sees gossiped RawTx, offers task for Alice to Charlie.
        // 4. Charlie assigns task from L2 to Alice. (Task now in Charlie's RawTxMempoolEntry.validation_tasks)
        // 5. Alice (Node A's client) polls Charlie, gets task (with task_id, generated_by_leader_id = L2's PK).
        // 6. Alice completes, signs, sends UserValidationTaskCompletion to L2.
        // This test spawn can only do step 6 if it knows these details.

        // Simplification: if this node (Alice) has any tasks assigned to it (as Charlie),
        // it tries to complete the first one. This is not how it would work.
        // The completion should be sent to the task GENERATOR (L2).

        // This simulation is too complex to get right without more infrastructure.
        // We will just log that the simulation channel exists.
        println!("ALICE SIM: Test task completion simulation channel is active. Manual trigger or more complex test needed.");

    });


    loop {
        select! {
            Some(gossiped_entry) = swarm.behaviour_mut().gossiped_tx_receiver.recv() => {
                println!("MainLoop: Received gossiped_entry for tx_id {} from channel. Processing.", gossiped_entry.tx_data.calculate_hash());
                // When a leader processes a gossiped raw_tx, it should offer tasks.
                let raw_tx_clone_for_task_offer = gossiped_entry.clone(); // Clone for separate async task
                let mut behaviour_clone_for_task_offer = swarm.behaviour_mut(); // Not ideal to hold &mut here

                // Process the gossiped transaction first
                if let Err(e) = behaviour_clone_for_task_offer.handle_gossiped_raw_transaction(gossiped_entry).await {
                    eprintln!("Error handling gossiped raw transaction: {}", e);
                }
                // Then, if this node is a leader, it should generate and offer tasks
                // This needs to be done carefully to avoid holding &mut swarm across .await points if not necessary
                // or by re-borrowing.
                // The generate_and_offer_task_for_raw_tx itself is async.
                // This could be a separate tokio::spawn if it doesn't need &mut self for too long,
                // or if its state interactions are through Arcs.
                // For now, direct call, assuming structure allows.
                swarm.behaviour_mut().generate_and_offer_task_for_raw_tx(&raw_tx_clone_for_task_offer).await;

            },
            Some(message) = swarm.behaviour_mut().offer_val_task_receiver.recv() => {
                if let P2PMessage::OfferValidationTaskToOriginLeader{ task } = message {
                    println!("MainLoop: Received OfferValidationTaskToOriginLeader for task_id {} (raw_tx_id {}). Processing.", task.task_id, task.raw_tx_id);
                    swarm.behaviour_mut().handle_offered_validation_task(task).await;
                }
            },
            Some(message) = swarm.behaviour_mut().user_task_completion_receiver.recv() => {
                if let P2PMessage::UserValidationTaskCompletion{ task_id, raw_tx_id, user_public_key_hex, completion_signature_bytes, completion_timestamp } = message {
                     println!("MainLoop: Received UserValidationTaskCompletion for task_id {} (raw_tx_id {}). Processing.", task_id, raw_tx_id);
                    swarm.behaviour_mut().handle_user_task_completion(task_id, raw_tx_id, user_public_key_hex, completion_signature_bytes, completion_timestamp).await;
                }
            },
            Some(message) = swarm.behaviour_mut().forwarded_completion_receiver.recv() => {
                 if let P2PMessage::ForwardUserTaskCompletionToOriginLeader{ completed_task } = message {
                    println!("MainLoop: Received ForwardUserTaskCompletionToOriginLeader for task_id {} (raw_tx_id {}). Processing.", completed_task.task_id, completed_task.raw_tx_id);
                    swarm.behaviour_mut().handle_forwarded_user_task_completion(completed_task).await;
                }
            },
            Some(message) = swarm.behaviour_mut().simulate_alice_completion_receiver.recv() => {
                 if let P2PMessage::UserValidationTaskCompletion{ task_id, raw_tx_id, user_public_key_hex, completion_signature_bytes, completion_timestamp } = message {
                     println!("MainLoop (TEST): Received SIMULATED UserValidationTaskCompletion for task_id {} (raw_tx_id {}). Processing.", task_id, raw_tx_id);
                    swarm.behaviour_mut().handle_user_task_completion(task_id, raw_tx_id, user_public_key_hex, completion_signature_bytes, completion_timestamp).await;
                }
            },

            _ = pulse_interval.tick() => { /* ... */ },
            _ = prune_interval.tick() => { /* ... */ },
            _ = leader_election_interval.tick() => { /* ... */ },

            _ = periodic_processing_tick.tick() => {
                // Leader election state machine processing (existing)
                let mut behaviour_for_election = swarm.behaviour_mut();
                if *behaviour_for_election.election_in_progress.lock().await {
                    // ... (election logic as before)
                }
                drop(behaviour_for_election);

                // Charlie processes its originated raw transactions (Step 5) (existing)
                let db_clone_step5 = swarm.behaviour().db.clone();
                let local_pk_hex_clone_step5 = swarm.behaviour().node_identity.public_key_hex.clone();
                let mut raw_tx_ids_to_process_step5 = Vec::new();
                // ... (logic to find raw_tx_ids for step 5 as before)
                 let iter_step5 = db_clone_step5.prefix_iterator(DB_RAW_TX_MEMPOOL_PREFIX.as_bytes());
                for item in iter_step5 {
                    if let Ok((key_bytes, value_bytes)) = item {
                        let key_str = String::from_utf8_lossy(&key_bytes);
                        let raw_tx_id_from_key = key_str.trim_start_matches(DB_RAW_TX_MEMPOOL_PREFIX);
                        if let Ok(entry) = serde_json::from_slice::<RawTxMempoolEntry>(&value_bytes) {
                            if entry.leader_id == local_pk_hex_clone_step5 {
                                let all_user_tasks_complete = entry.validation_tasks.values()
                                    .filter(|task| matches!(task.task_type, ValidationTaskType::UserSignatureAndBalanceValidation) &&
                                                   task.assigned_to_user_pk_hex.is_some())
                                    .all(|task| task.completed);
                                if all_user_tasks_complete && entry.validation_timestamps.len() >= MIN_VALIDATION_TIMESTAMPS_FOR_PROCESSING {
                                    raw_tx_ids_to_process_step5.push(raw_tx_id_from_key.to_string());
                                }
                            }
                        }
                    }
                }
                drop(db_clone_step5);
                for raw_tx_id in raw_tx_ids_to_process_step5 {
                    if let Err(e) = swarm.behaviour_mut().attempt_process_raw_transaction(&raw_tx_id).await {
                        eprintln!("Error in Step 5 processing for raw_tx {}: {}", raw_tx_id, e);
                    }
                }

                // Node (acting as Validator) picks up LeaderTimestampMathCheck tasks (Step 6 Part 1)
                let db_clone_step6 = swarm.behaviour().db.clone();
                let mut proctx_ids_for_math_check = Vec::new();
                let iter_step6 = db_clone_step6.prefix_iterator(DB_VALIDATION_TASKS_MEMPOOL_PREFIX.as_bytes());
                for item in iter_step6 {
                     if let Ok((key_bytes, value_bytes)) = item {
                        // Key is "valtask_<proctx_id>" for these tasks
                        // Value is serialized ValidationTask
                        if let Ok(task_details) = serde_json::from_slice::<ValidationTask>(&value_bytes) {
                            if matches!(task_details.task_type, ValidationTaskType::LeaderTimestampMathCheck) &&
                               !task_details.completed { // Only pick up incomplete tasks
                                proctx_ids_for_math_check.push(task_details.subject_tx_id.clone());
                            }
                        }
                    }
                }
                drop(db_clone_step6);
                for proctx_id in proctx_ids_for_math_check {
                     println!("PeriodicTick: Node acting as validator, attempting math check for proctx_id: {}", proctx_id);
                    if let Err(e) = swarm.behaviour_mut().simulate_validator_completing_math_check(&proctx_id).await {
                        eprintln!("Error simulating validator math check for proctx {}: {}", proctx_id, e);
                    }
                }
            },
            Some(message) = swarm.behaviour_mut().verified_processing_tx_receiver.recv() => {
                if let P2PMessage::VerifiedProcessingTxBroadcast{ processing_entry, validator_id_pk_hex, validator_signature_on_tx_id } = message {
                    println!("MainLoop: Received VerifiedProcessingTxBroadcast for proctx_id {} from validator {}. Processing.",
                        processing_entry.tx_id, validator_id_pk_hex);
                    if let Err(e) = swarm.behaviour_mut().handle_verified_processing_tx_broadcast(
                        processing_entry, validator_id_pk_hex, validator_signature_on_tx_id).await {
                        eprintln!("Error handling verified processing tx broadcast: {}", e);
                    }
                }
            },
            Some(client_tx_data) = swarm.behaviour_mut().client_submitted_tx_receiver.recv() => {
                println!("MainLoop: Received ClientSubmitRawTransaction with TxData for user {}. Processing.", client_tx_data.user);
                // This node must be a leader to process it.
                // The handle_incoming_raw_transaction function already checks for leadership.
                if let Err(e) = swarm.behaviour_mut().handle_incoming_raw_transaction(client_tx_data).await {
                    eprintln!("Error handling client submitted raw transaction: {}", e);
                }
            },
            // Existing MPSC channel handlers...
            // Some(gossiped_entry) = swarm.behaviour_mut().gossiped_tx_receiver.recv() => { /* ... as before ... */ }, // This is duplicated below, removing one.
            Some(message) = swarm.behaviour_mut().offer_val_task_receiver.recv() => { /* ... as before ... */ },
            Some(message) = swarm.behaviour_mut().user_task_completion_receiver.recv() => { /* ... as before ... */ },
            Some(message) = swarm.behaviour_mut().forwarded_completion_receiver.recv() => { /* ... as before ... */ },
            Some(message) = swarm.behaviour_mut().simulate_alice_completion_receiver.recv() => { /* ... as before ... */ },

            event = swarm.select_next_some() => {
                match event {
                    SwarmEvent::NewListenAddr { address, .. } => { println!("Listening on {}", address); }
                    SwarmEvent::Behaviour(_event) => { /* ... */ }
                    SwarmEvent::ConnectionEstablished { peer_id, .. } => { println!("Connection established with: {}", peer_id); }
                    SwarmEvent::ConnectionClosed { peer_id, cause, .. } => { println!("Connection to {} closed, cause: {:?}", peer_id, cause); }
                    SwarmEvent::IncomingConnection { local_addr, send_back_addr } => { println!("Incoming connection from {} to {}", send_back_addr, local_addr); }
                    SwarmEvent::IncomingConnectionError { local_addr, send_back_addr, error } => { eprintln!("Incoming connection error from {} to {}: {}", send_back_addr, local_addr, error); }
                    SwarmEvent::OutgoingConnectionError { peer_id, error } => { eprintln!("Outgoing connection error to {:?}: {}", peer_id, error); }
                    SwarmEvent::Dialing(peer_id) => { println!("Dialing {}", peer_id); }
                     _ => {}
                }
            }
        }
    }
}
