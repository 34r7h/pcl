use thiserror::Error;

#[derive(Error, Debug)]
pub enum PclError {
    #[error("Node identity error: {0}")]
    NodeIdentity(String),
    
    #[error("Signature verification failed: {0}")]
    SignatureVerification(String),
    
    #[error("IP address validation failed: {0}")]
    IpValidation(String),
    
    #[error("Mempool error: {0}")]
    Mempool(String),
    
    #[error("Transaction error: {0}")]
    Transaction(String),
    
    #[error("Network error: {0}")]
    Network(String),
    
    #[error("Storage error: {0}")]
    Storage(String),
    
    #[error("Consensus error: {0}")]
    Consensus(String),
    
    #[error("Validation error: {0}")]
    Validation(String),
    
    #[error("Serialization error: {0}")]
    Serialization(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("RocksDB error: {0}")]
    RocksDb(#[from] rocksdb::Error),
    
    #[error("Serde JSON error: {0}")]
    SerdeJson(#[from] serde_json::Error),
    
    #[error("Bincode error: {0}")]
    Bincode(#[from] bincode::Error),
    
    #[error("Libp2p error: {0}")]
    Libp2p(String),
}

impl From<libp2p::swarm::ConnectionDenied> for PclError {
    fn from(error: libp2p::swarm::ConnectionDenied) -> Self {
        PclError::Network(error.to_string())
    }
}

pub type Result<T> = std::result::Result<T, PclError>; 