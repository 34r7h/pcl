use consensus_node_lib::data_structures::TxData;
use crate::user_manager::SimulatedUser;
use crate::SimulatorConfig; // Assuming SimulatorConfig is in scope, likely from main.rs or lib.rs
use rand::Rng;
use std::collections::HashMap;
use chrono::Utc;
use ed25519_dalek::Keypair; // For direct access if TxData signing takes Keypair ref

pub struct TransactionGenerator {}

impl TransactionGenerator {
    pub fn new() -> Self {
        TransactionGenerator {}
    }

    pub fn generate_transaction(
        &self,
        sender: &SimulatedUser,
        // For simplicity, let's assume recipient is also a SimulatedUser for now,
        // or just a public key hex string.
        recipient_pk_hex: String,
        config: &SimulatorConfig,
        tx_counter: u64, // A counter to make UTXO IDs unique for a sender
    ) -> TxData {
        let mut rng = rand::thread_rng();

        let amount_to_send = rng.gen_range(config.tx_amount_min..=config.tx_amount_max);

        // For simplicity, assume the 'from' UTXO has enough balance.
        // In a real system, UTXOs would be tracked. Here, we generate a dummy one.
        // The amount in the 'from' UTXO should be >= amount_to_send + fee + stake (for change calculation if any)
        // Let's make the dummy UTXO have amount_to_send + fee + stake + some_change_buffer
        let fee = rng.gen_range(config.tx_fee_min..=config.tx_fee_max);
        let stake = rng.gen_range(config.tx_stake_min..=config.tx_stake_max);
        let from_utxo_amount = amount_to_send + fee.ceil() as u64 + stake.ceil() as u64 + rng.gen_range(1..=100); // Ensure enough for fee and stake

        // Create a unique dummy UTXO ID for this transaction from this sender
        let from_utxo_id = format!("sim_utxo_{}_{}", sender.public_key_hex, tx_counter);

        let mut to_map = HashMap::new();
        to_map.insert(recipient_pk_hex, amount_to_send);

        let mut from_map = HashMap::new();
        from_map.insert(from_utxo_id, from_utxo_amount);

        let tx_data_unsigned = TxData {
            to: to_map,
            from: from_map,
            user: sender.public_key_hex.clone(),
            signature_bytes: Vec::new(), // Will be filled by sign method
            stake,
            fee,
            timestamp: Utc::now(),
        };

        // Sign the transaction
        // The TxData::sign method in consensus_node takes &Keypair
        tx_data_unsigned.sign(&sender.keypair)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::user_manager::UserManager; // Adjusted path assuming main.rs or lib.rs declares modules
    use clap::Parser; // For creating a dummy config

    // Helper to create a default config for testing
    fn default_test_config() -> SimulatorConfig {
        SimulatorConfig::parse_from(Vec::<String>::new()) // Parses defaults
    }

    #[test]
    fn test_generate_single_transaction() {
        let user_manager = UserManager::new(2);
        let sender = user_manager.get_next_user();
        let recipient = user_manager.get_next_user(); // Could be the same if only 1 user

        let config = default_test_config();
        let generator = TransactionGenerator::new();

        let tx = generator.generate_transaction(sender, recipient.public_key_hex.clone(), &config, 1);

        assert_eq!(tx.user, sender.public_key_hex);
        assert!(!tx.signature_bytes.is_empty());
        assert_eq!(tx.to.keys().next().unwrap(), &recipient.public_key_hex);
        assert!(tx.to.values().next().unwrap() >= &config.tx_amount_min);
        assert!(tx.to.values().next().unwrap() <= &config.tx_amount_max);
        assert!(tx.fee >= config.tx_fee_min && tx.fee <= config.tx_fee_max);
        assert!(tx.stake >= config.tx_stake_min && tx.stake <= config.tx_stake_max);

        // Verify signature
        let sender_public_key = sender.keypair.public;
        assert!(tx.verify_signature(&sender_public_key));
    }

    #[test]
    fn test_transaction_utxo_id_uniqueness_per_sender() {
        let user_manager = UserManager::new(1); // Single sender
        let sender = user_manager.get_next_user();
        let recipient_pk_hex = "dummy_recipient_pk_hex".to_string();

        let config = default_test_config();
        let generator = TransactionGenerator::new();

        let tx1 = generator.generate_transaction(sender, recipient_pk_hex.clone(), &config, 1);
        let tx2 = generator.generate_transaction(sender, recipient_pk_hex.clone(), &config, 2);
        let tx3 = generator.generate_transaction(sender, recipient_pk_hex.clone(), &config, 100);

        let utxo1 = tx1.from.keys().next().unwrap();
        let utxo2 = tx2.from.keys().next().unwrap();
        let utxo3 = tx3.from.keys().next().unwrap();

        assert_ne!(utxo1, utxo2);
        assert_ne!(utxo1, utxo3);
        assert_ne!(utxo2, utxo3);

        assert!(utxo1.starts_with(&format!("sim_utxo_{}", sender.public_key_hex)));
        assert!(utxo1.ends_with("_1"));
        assert!(utxo2.ends_with("_2"));
        assert!(utxo3.ends_with("_100"));
    }
}
```
