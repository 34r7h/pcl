// This lib.rs is created to allow the `consensus_simulator` to access common structures.

// Make the data_structures module public so it can be used by other crates.
pub mod data_structures;

// Potentially, other modules could be made public here if needed by the simulator
// or other external tools. For now, only data_structures is strictly required
// for sharing P2PMessage, TxData, etc.
// pub mod p2p; // Example: if p2p contained reusable components, but likely not needed for simulator.
```
