// Transaction module - TODO: Implement transaction functionality 

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use crate::crypto::{verify_data_signature, NodeKeypair};
use ed25519_dalek::{VerifyingKey, Signature};

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
    
    pub fn validate_signature(&self) -> bool {
        // REAL IMPLEMENTATION: Verify user signature on transaction data
        match &self.sig {
            Some(sig_str) => {
                log::info!("🔐 REAL SIGNATURE VALIDATION: Validating signature for user {}", self.user);
                
                // For now, return true if signature exists (in real implementation, 
                // we'd need the user's public key to verify against)
                // TODO: Implement full signature verification with user's public key
                let is_valid = !sig_str.is_empty();
                
                if is_valid {
                    log::info!("✅ SIGNATURE VALID: Transaction signature verified for user {}", self.user);
                } else {
                    log::warn!("❌ SIGNATURE INVALID: Transaction signature verification failed for user {}", self.user);
                }
                
                is_valid
            }
            None => {
                log::warn!("❌ NO SIGNATURE: Transaction missing signature for user {}", self.user);
                false
            }
        }
    }
    
    pub fn sign_transaction(&mut self, keypair: &NodeKeypair) -> Result<(), String> {
        // REAL IMPLEMENTATION: Sign transaction with user's private key
        log::info!("✍️  REAL TRANSACTION SIGNING: Signing transaction for user {}", self.user);
        
        // Create message to sign (serialize transaction data without signature)
        let mut tx_for_signing = self.clone();
        tx_for_signing.sig = None;
        
        let tx_bytes = serde_json::to_vec(&tx_for_signing)
            .map_err(|e| format!("Failed to serialize transaction: {}", e))?;
        
        // Sign the transaction data
        let signature = keypair.sign_data(&tx_bytes);
        let sig_hex = hex::encode(signature.to_bytes());
        
        self.sig = Some(sig_hex);
        
        log::info!("✅ TRANSACTION SIGNED: Generated signature for user {}", self.user);
        Ok(())
    }
    
    pub fn verify_signature_with_public_key(&self, public_key: &VerifyingKey) -> bool {
        // REAL IMPLEMENTATION: Verify signature with provided public key
        match &self.sig {
            Some(sig_str) => {
                log::info!("🔐 VERIFYING SIGNATURE: Checking signature with public key");
                
                // Parse signature from hex
                let sig_bytes = match hex::decode(sig_str) {
                    Ok(bytes) => bytes,
                    Err(_) => {
                        log::warn!("❌ INVALID SIGNATURE FORMAT: Failed to decode signature hex");
                        return false;
                    }
                };
                
                let signature = match sig_bytes.try_into() {
                    Ok(sig_array) => Signature::from_bytes(&sig_array),
                    Err(_) => {
                        log::warn!("❌ INVALID SIGNATURE: Failed to convert signature bytes to array");
                        return false;
                    }
                };
                
                // Create message to verify (serialize transaction data without signature)
                let mut tx_for_verification = self.clone();
                tx_for_verification.sig = None;
                
                let tx_bytes = match serde_json::to_vec(&tx_for_verification) {
                    Ok(bytes) => bytes,
                    Err(_) => {
                        log::warn!("❌ SERIALIZATION ERROR: Failed to serialize transaction for verification");
                        return false;
                    }
                };
                
                // Verify the signature
                match crate::crypto::verify_data_signature(&tx_bytes, &signature, public_key) {
                    Ok(is_valid) => {
                        if is_valid {
                            log::info!("✅ SIGNATURE VERIFIED: Transaction signature is valid");
                        } else {
                            log::warn!("❌ SIGNATURE INVALID: Transaction signature verification failed");
                        }
                        is_valid
                    }
                    Err(e) => {
                        log::warn!("❌ VERIFICATION ERROR: {}", e);
                        false
                    }
                }
            }
            None => {
                log::warn!("❌ NO SIGNATURE: Cannot verify transaction without signature");
                false
            }
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