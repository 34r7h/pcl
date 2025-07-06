use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer, Verifier};
use rand::{rngs::OsRng, RngCore};
use sha2::{Sha256, Digest};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use crate::error::{PclError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeKeypair {
    pub signing_key: SigningKey,
}

impl Default for NodeKeypair {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeKeypair {
    pub fn new() -> Self {
        let mut csprng = OsRng;
        let mut secret_key = [0u8; 32];
        rand::RngCore::fill_bytes(&mut csprng, &mut secret_key);
        let signing_key = SigningKey::from_bytes(&secret_key);
        log::info!("Generated new node keypair with public key: {:?}", signing_key.verifying_key());
        Self { signing_key }
    }

    pub fn from_bytes(secret_bytes: &[u8]) -> Result<Self> {
        let signing_key = SigningKey::from_bytes(secret_bytes.try_into()
            .map_err(|_| PclError::NodeIdentity("Invalid secret key length".to_string()))?);
        log::info!("Created node keypair from bytes with public key: {:?}", signing_key.verifying_key());
        Ok(Self { signing_key })
    }

    pub fn public_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    pub fn sign_ip_address(&self, ip: &IpAddr) -> Result<Signature> {
        let ip_bytes = match ip {
            IpAddr::V4(ipv4) => ipv4.octets().to_vec(),
            IpAddr::V6(ipv6) => ipv6.octets().to_vec(),
        };
        
        let mut hasher = Sha256::new();
        hasher.update(&ip_bytes);
        let hash = hasher.finalize();
        
        let signature = self.signing_key.sign(&hash);
        log::info!("Signed IP address {} with signature: {:?}", ip, signature);
        Ok(signature)
    }

    pub fn sign_data(&self, data: &[u8]) -> Signature {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let hash = hasher.finalize();
        
        let signature = self.signing_key.sign(&hash);
        log::debug!("Signed data with signature: {:?}", signature);
        signature
    }
}

pub fn verify_ip_signature(ip: &IpAddr, signature: &Signature, public_key: &VerifyingKey) -> Result<bool> {
    let ip_bytes = match ip {
        IpAddr::V4(ipv4) => ipv4.octets().to_vec(),
        IpAddr::V6(ipv6) => ipv6.octets().to_vec(),
    };
    
    let mut hasher = Sha256::new();
    hasher.update(&ip_bytes);
    let hash = hasher.finalize();
    
    match public_key.verify(&hash, signature) {
        Ok(()) => {
            log::info!("IP signature verification successful for IP: {}", ip);
            Ok(true)
        }
        Err(e) => {
            log::warn!("IP signature verification failed for IP: {}, error: {}", ip, e);
            Ok(false)
        }
    }
}

pub fn verify_data_signature(data: &[u8], signature: &Signature, public_key: &VerifyingKey) -> Result<bool> {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let hash = hasher.finalize();
    
    match public_key.verify(&hash, signature) {
        Ok(()) => {
            log::debug!("Data signature verification successful");
            Ok(true)
        }
        Err(e) => {
            log::warn!("Data signature verification failed: {}", e);
            Ok(false)
        }
    }
}

pub fn hash_transaction_data(data: &[u8]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().to_vec()
}

pub fn calculate_digital_root(tx_id: &[u8]) -> u8 {
    let sum: u32 = tx_id.iter().map(|&b| b as u32).sum();
    let mut digital_root = sum;
    
    while digital_root >= 10 {
        digital_root = digital_root.to_string()
            .chars()
            .map(|c| c.to_digit(10).unwrap())
            .sum();
    }
    
    log::debug!("Calculated digital root: {} for tx_id sum: {}", digital_root, sum);
    digital_root as u8
}

// Convenience functions for simulator compatibility
pub fn generate_keypair() -> NodeKeypair {
    NodeKeypair::new()
}

pub fn sign_data(keypair: &NodeKeypair, data: &[u8]) -> Signature {
    keypair.sign_data(data)
}

pub fn hash_data(data: &[u8]) -> Vec<u8> {
    hash_transaction_data(data)
} 