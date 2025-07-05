# Peer Consensus Layer Test Suite

## Overview
This test suite provides comprehensive coverage for the Peer Consensus Layer (PCL) system as described in the README. The tests are designed using Test-Driven Development (TDD) principles to guide the implementation of the entire system.

## Test Structure

### 1. Node Identity Tests (`tests/node_identity.rs`)
- **Node Creation**: IP address signing with private keys
- **Identity Validation**: Signature verification
- **IP Address Validation**: Format validation for IPv4/IPv6
- **Node Roles**: Extension vs Leader vs Validator roles
- **Family Management**: Node family assignment for pulse system
- **Disqualification Tracking**: 24-hour disqualification periods

**Total Tests**: 10 tests covering complete node identity management

### 2. Mempool Tests (`tests/mempool.rs`)
- **Raw Transaction Mempool**: Entry creation, hashing, removal
- **Validation Tasks Mempool**: Task assignment, completion tracking
- **Locked UTXO Mempool**: Double-spend prevention, conflict detection
- **Processing Transaction Mempool**: Timestamp averaging, leader signatures
- **Final Transaction Mempool**: XMBL integration, UTXO creation
- **Uptime Mempool**: Pulse tracking, response time averaging
- **RocksDB Integration**: Persistence and recovery
- **Invalidation Handling**: Cleanup and gossiping

**Total Tests**: 26 tests covering all mempool types and operations

### 3. Transaction Workflow Tests (`tests/transaction_workflow.rs`)
- **Step 1**: Transaction creation and validation
- **Step 2**: Leader processing and gossiping
- **Step 3**: Validation task assignment
- **Step 4**: Task completion by validators
- **Step 5**: Transaction processing and signing
- **Step 6**: Final validation and UTXO creation
- **Invalidation Handling**: Cleanup at each step
- **End-to-End**: Complete workflow testing

**Total Tests**: 32 tests covering complete transaction lifecycle

### 4. Leader Election Tests (`tests/leader_election.rs`)
- **Pulse System**: 20-second pulse intervals, family communication
- **Uptime Tracking**: Mempool management, response time averaging
- **Broadcasting**: 2-hour uptime data broadcasts
- **Nomination**: Performance-based leader selection
- **Voting System**: 3-round run-off voting
- **List Management**: Leader sorting and broadcasting
- **Disqualification**: Non-participation penalties
- **Fault Tolerance**: Byzantine fault handling

**Total Tests**: 30 tests covering complete leader election cycle

### 5. Network Communication Tests (`tests/network_communication.rs`)
- **Gossiping Protocol**: 3-leader propagation, loop prevention
- **Broadcasting**: Reliable message delivery
- **Libp2p Integration**: Peer discovery, connection management
- **Pulse Communication**: Family member messaging
- **Validation Communication**: Task assignment and reporting
- **Network Resilience**: Partition tolerance, fault recovery
- **Performance**: Latency optimization, bandwidth efficiency
- **Security**: Authentication, encryption, attack prevention

**Total Tests**: 30 tests covering all network communication aspects

### 6. Integration Tests (`tests/integration.rs`)
- **RocksDB Integration**: Persistence, recovery, performance
- **XMBL Cubic DLT**: Digital root calculation, geometry inclusion
- **End-to-End**: Complete system workflows
- **Stress Testing**: High load, concurrent operations
- **Fault Tolerance**: Node failures, network partitions
- **Data Consistency**: Mempool, transaction, UTXO consistency
- **Performance Benchmarking**: Throughput, latency, scalability
- **Security**: Cryptographic validation, attack prevention
- **Component Integration**: Extension, simulator integration

**Total Tests**: 36 tests covering complete system integration

## Test Execution

### Running All Tests
```bash
cd backend
cargo test
```

### Running Specific Test Modules
```bash
# Node identity tests
cargo test --test node_identity

# Mempool tests  
cargo test --test mempool

# Transaction workflow tests
cargo test --test transaction_workflow

# Leader election tests
cargo test --test leader_election

# Network communication tests
cargo test --test network_communication

# Integration tests
cargo test --test integration
```

### Running Specific Test Categories
```bash
# All validation tests
cargo test validation

# All mempool tests
cargo test mempool

# All leader election tests
cargo test leader_election

# All network tests
cargo test network
```

### Running Benchmarks
```bash
# Transaction throughput benchmark
cargo bench transaction_throughput

# Leader election latency benchmark
cargo bench leader_election_latency

# Mempool performance benchmark
cargo bench mempool_performance
```

## Test Coverage Summary

| Component | Tests | Key Areas |
|-----------|-------|-----------|
| Node Identity | 10 | Identity, roles, families, disqualification |
| Mempool | 26 | All 6 mempool types, persistence, invalidation |
| Transaction Workflow | 32 | Complete 6-step workflow, invalidation handling |
| Leader Election | 30 | Pulse system, voting, fault tolerance |
| Network Communication | 30 | Gossiping, broadcasting, libp2p, security |
| Integration | 36 | RocksDB, XMBL, end-to-end, benchmarking |
| **Total** | **164** | **Complete system coverage** |

## Expected Outcomes

Each test includes:
- **Description**: Clear test purpose
- **Expected Behavior**: What should happen
- **Implementation Guidance**: How to build the feature

### Log Format
All tests output expected behaviors:
```
Expected: [Specific expected outcome for this test]
```

## Implementation Workflow

1. **Run tests** to see current failures
2. **Implement features** to make tests pass
3. **Add logging** as specified in test descriptions
4. **Iterate** until all tests pass
5. **Run benchmarks** to validate performance
6. **Integration testing** with all components

## Key System Components to Implement

Based on the test suite, you'll need to build:

1. **Node Management System**
   - Identity creation and validation
   - Role assignment (Extension/Leader/Validator)
   - Family management for pulse system

2. **Mempool Management**
   - 6 different mempool types
   - RocksDB persistence layer
   - Invalidation handling

3. **Transaction Processing**
   - 6-step workflow implementation
   - Validation task system
   - UTXO management

4. **Leader Election System**
   - Pulse and response tracking
   - Performance-based nomination
   - 3-round voting system

5. **Network Layer**
   - Libp2p-based communication
   - Gossiping and broadcasting protocols
   - Security and fault tolerance

6. **Integration Layer**
   - XMBL Cubic DLT integration
   - Extension and simulator interfaces
   - Performance monitoring

## Dependencies

The test suite expects these major dependencies:
- `ed25519-dalek` for cryptographic signatures
- `libp2p` for peer-to-peer networking
- `rocksdb` for persistent storage
- `tokio` for async runtime
- `serde` for serialization
- `chrono` for timestamp handling

## Next Steps

1. Run the test suite to see current state
2. Begin implementing core data structures
3. Add logging as specified in tests
4. Implement components incrementally
5. Use tests to guide development
6. Run benchmarks for performance validation

The test suite is designed to be your comprehensive guide for building a robust, scalable peer consensus layer system. 