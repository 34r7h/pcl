// Transaction module - TODO: Implement transaction functionality 

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionData {
    pub to: Vec<(String, f64)>,  // (address, amount) pairs
    pub from: Vec<(String, f64)>, // (utxo_id, amount) pairs
    pub user: String,            // sender address
    pub sig: Option<String>,     // signature (signed message without sig property)
    pub stake: f64,             // validation stake
    pub fee: f64,               // transaction fee
    pub change: Option<f64>,    // change amount
    pub timestamp: DateTime<Utc>,
    pub leader: Option<String>,  // leader node IP
    pub nonce: u64,             // transaction nonce
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawTransaction {
    pub raw_tx_id: String,
    pub tx_data: TransactionData,
    pub validation_timestamps: Vec<DateTime<Utc>>,
    pub validation_tasks: Vec<ValidationTask>,
    pub tx_timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationTask {
    pub task_id: String,
    pub leader_id: String,
    pub task_type: ValidationTaskType,
    pub complete: bool,
    pub assigned_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationTaskType {
    SignatureValidation,
    SpendingPowerValidation,
    TimestampValidation,
    MathValidation,
    FinalValidation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingTransaction {
    pub tx_id: String,
    pub tx_data: TransactionData,
    pub sig: String,            // leader signature
    pub leader: String,         // leader node ID
    pub timestamp: DateTime<Utc>, // averaged timestamp
}

impl TransactionData {
    pub fn new(
        to: Vec<(String, f64)>,
        from: Vec<(String, f64)>,
        user: String,
        stake: f64,
        fee: f64,
    ) -> Self {
        let total_from: f64 = from.iter().map(|(_, amount)| amount).sum();
        let total_to: f64 = to.iter().map(|(_, amount)| amount).sum();
        let change = total_from - total_to - stake - fee;
        
        Self {
            to,
            from,
            user,
            sig: None,
            stake,
            fee,
            change: if change > 0.0 { Some(change) } else { None },
            timestamp: Utc::now(),
            leader: None,
            nonce: 0,
        }
    }
    
    pub fn set_leader(&mut self, leader_ip: String) {
        self.leader = Some(leader_ip);
    }
    
    pub fn set_signature(&mut self, signature: String) {
        self.sig = Some(signature);
    }
    
    pub fn set_nonce(&mut self, nonce: u64) {
        self.nonce = nonce;
    }
    
    pub fn validate_amounts(&self) -> bool {
        let total_from: f64 = self.from.iter().map(|(_, amount)| amount).sum();
        let total_to: f64 = self.to.iter().map(|(_, amount)| amount).sum();
        let total_out = total_to + self.stake + self.fee;
        
        if let Some(change) = self.change {
            total_from >= total_out + change
        } else {
            total_from >= total_out
        }
    }
    
    pub fn get_bytes_for_signing(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut data_to_sign = self.clone();
        data_to_sign.sig = None; // Signature must not be part of the signed payload
        serde_json::to_vec(&data_to_sign)
    }

    pub fn validate_signature(&self, public_key: &ed25519_dalek::VerifyingKey) -> bool {
        if self.sig.is_none() {
            return false;
        }
        let signature_bytes = match hex::decode(self.sig.as_ref().unwrap()) {
            Ok(bytes) => bytes,
            Err(_) => return false, // Invalid hex for signature
        };
        let signature = match ed25519_dalek::Signature::from_bytes(&signature_bytes) {
            Ok(sig) => sig,
            Err(_) => return false, // Invalid signature format
        };

        match self.get_bytes_for_signing() {
            Ok(message_bytes) => {
                // Assuming crypto::verify_data_signature is available and correct
                // We need to ensure the public_key is correctly retrieved for self.user
                // For now, this function takes public_key as an argument.
                // The caller (e.g., in consensus.rs) will be responsible for fetching the key.
                crate::crypto::verify_data_signature(&message_bytes, &signature, public_key).unwrap_or(false)
            }
            Err(_) => false, // Serialization error
        }
    }
    
    pub fn get_total_amount(&self) -> f64 {
        self.to.iter().map(|(_, amount)| amount).sum()
    }
    
    pub fn get_total_input(&self) -> f64 {
        self.from.iter().map(|(_, amount)| amount).sum()
    }
    
    pub fn calculate_digital_root(&self) -> u32 {
        // Calculate digital root for XMBL Cubic DLT
        let total = (self.get_total_amount() * 1000.0) as u32;
        let mut sum = total;
        while sum >= 10 {
            let mut temp = 0;
            while sum > 0 {
                temp += sum % 10;
                sum /= 10;
            }
            sum = temp;
        }
        sum
    }
}

impl RawTransaction {
    pub fn new(raw_tx_id: String, tx_data: TransactionData) -> Self {
        Self {
            raw_tx_id,
            tx_data,
            validation_timestamps: Vec::new(),
            validation_tasks: Vec::new(),
            tx_timestamp: Utc::now(),
        }
    }
    
    pub fn add_validation_task(&mut self, task: ValidationTask) {
        self.validation_tasks.push(task);
    }
    
    pub fn complete_validation_task(&mut self, task_id: &str) -> bool {
        for task in &mut self.validation_tasks {
            if task.task_id == task_id {
                task.complete = true;
                task.completed_at = Some(Utc::now());
                return true;
            }
        }
        false
    }
    
    pub fn add_validation_timestamp(&mut self, timestamp: DateTime<Utc>) {
        self.validation_timestamps.push(timestamp);
    }
    
    pub fn get_average_timestamp(&self) -> Option<DateTime<Utc>> {
        if self.validation_timestamps.is_empty() {
            return None;
        }
        
        let total_seconds: i64 = self.validation_timestamps
            .iter()
            .map(|dt| dt.timestamp())
            .sum();
        let average_seconds = total_seconds / self.validation_timestamps.len() as i64;
        
        Some(DateTime::from_timestamp(average_seconds, 0).unwrap_or(Utc::now()))
    }
    
    pub fn is_validation_complete(&self) -> bool {
        !self.validation_tasks.is_empty() && 
        self.validation_tasks.iter().all(|task| task.complete)
    }
}

impl ValidationTask {
    pub fn new(task_id: String, leader_id: String, task_type: ValidationTaskType) -> Self {
        Self {
            task_id,
            leader_id,
            task_type,
            complete: false,
            assigned_at: Utc::now(),
            completed_at: None,
        }
    }
    
    pub fn complete(&mut self) {
        self.complete = true;
        self.completed_at = Some(Utc::now());
    }
    
    pub fn is_expired(&self, timeout_minutes: i64) -> bool {
        let timeout = chrono::Duration::minutes(timeout_minutes);
        Utc::now() > self.assigned_at + timeout
    }
}

impl ProcessingTransaction {
    pub fn new(tx_id: String, tx_data: TransactionData, leader_sig: String, leader_id: String) -> Self {
        Self {
            tx_id,
            tx_data,
            sig: leader_sig,
            leader: leader_id,
            timestamp: Utc::now(),
        }
    }
    
    pub fn from_raw_transaction(raw_tx: &RawTransaction, leader_sig: String, leader_id: String) -> Option<Self> {
        let avg_timestamp = raw_tx.get_average_timestamp()?;
        
        let mut tx_data = raw_tx.tx_data.clone();
        tx_data.timestamp = avg_timestamp;
        
        Some(Self {
            tx_id: raw_tx.raw_tx_id.clone(),
            tx_data,
            sig: leader_sig,
            leader: leader_id,
            timestamp: avg_timestamp,
        })
    }
} 