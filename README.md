# Peer Consensus Layer

### Node Identity
A node is an IP address that is signed by a user's private key. Extensions may be nodes but it's unlikely they would be leaders. Nodes may be validators but only when there's heavy system load.

## Examples

### Alice makes a transaction

#### Mempools

a. raw_tx_mempool - first entries of tx requests
b. validation_tasks_mempool - validation tasks required to process transactions
c. locked_utxo_mempool - utxos in this list are invalidated from entry into raw_tx_mempool entries
d. processing_tx_mempool - transactions that moving through the consensus
e. tx_mempool - transactions approved to be blocked or otherwise finalized in a DLT's protocol

#### Workflow

1. Alice sends Bob a transaction of one coin to a leader node, Charlie. Alice includes the tx fee and a validation stake.
    
    tx_data = {
        to: [bob_address: 1],
        from: [alice_utxo1: 2],
        user: alice_address,
        sig: alice_signature, // signs this message, without the "sig" property
        stake: 0.2,
        fee: 0.1
    } // total of 1.3 coins required, .9 returns to alice_address as new utxo on finality.

2. Charlie hashes the raw transaction to get the raw_tx_id and starts a raw_tx_mempool entry under his node id property. Charlie also adds Alice's raw_tx_id to the validation_tasks_mempool. Charlie gossips to 3 leaders who continue to gossip the transaction to other leaders. At this point, UTXO's used in the transaction are put on the locked_utxo_mempool to prevent double-spend attacks. 

    raw_tx_mempool = {
        charlie_id: {
            raw_tx_id: {
                tx_data,
                validation_timestamps: [],
                validation_tasks: [],
                tx_timestamp: 1751728707356
            }
        }
    }

3. The other leaders send Charlie validation tasks for Alice to complete. Charlie approves validation tasks to send to Alice sorting by the tx_timestamp on other raw transactions. The number of tasks is proportionate to total tasks in validation_tasks_mempool divided by validators available. The first task type is validating Alice's signature and spending power, the timestamps from which go into the validation_timestamps array.

    validation_tasks = [
        leader2_id: [
            {task: task_id1, complete: false}, {task_id2, complete: false}
        ],
        leader8_id: [
            {task: task_id1, complete: false}, {task: task_id2, , complete: false}
        ]
    ]

4. Alice completes the assigned validation tasks and if valid, Alice reports the timestamp to the leaders with her signature on each completed task. Leader 2 and leader 8 in the above example report the completed timestamps and task_id to Charlie. Charlie marks validation tasks complete as true and removes Alice's raw_tx_id from the validation_tasks_mempool.

5. When tasks are complete and Alice's transaction's raw_tx_mempool entry has the required number of validating timestamps (set by network configuration), Charlie removes the transaction from raw_tx_mempool, averages the validation_timestamps, signs it, and puts it in processing_tx_mempool. Another task type is put in the validation_task_mempool to send to validators -- check Charlie's math from averaging timestamps and hash the {timestamp: tx_data} value to get Alice's tx_id. 

6. The validator who gets Alice's tx_id in Step 5 broadcasts the transaction to 3 random leaders to put in their processing_tx_mempool, add/check that finality validation tasks for their chain of choice. and then those leaders remove the entry in their raw_tx_mempool and associated validation_tasks_mempool entries. Leaders gossip the transaction to all other leaders.

    processing_tx_mempool: {
        tx_id: { // timestamp used is the average generated in step 5
            1751730407001: tx_data,
            sig: charlie_signature
            leader: charlie_id
        }
    }

6. The final validation task type depends on the chain. At this point Alice is provided with a new UTXO with her change and stake return. Bob's new UTXO is awaiting final validation. For our example, we will use the XMBL's Cubic DLT protocol's requirement to calculate the digital root of the tx_id and put into a tx_mempool for further inclusion in the protocol's cubic geometry.

In the case of a invalidation at any step, all entries into mempools are removed and sent to all leaders who gossip to nodes.

### A leader is born

Nodes send a pulse every 20 seconds to nodes in their family. A receiving node sets the ip address with a timestamp property in their uptime_mempool and sends a message back. If more than 60 seconds passes between pulses from any node in their family, the node is removed from their uptime_mempool. For every pulse received from a node after the timestamp is set, we update the count by 1. The time between receiving the response is averaged into the node's response time value (add the new response time to the running average and divide by the updated count to get the new average)

    {
        45.228.345: {
            1751739100498: [1, 201]
        }
    }

Every 2 hours nodes broadcast their uptime_mempool data to nodes and validators. Nodes and validators elect leaders based on most uptime and fastest response time averages. The nominated leaders are broadcast to all nodes who then do 3 rounds of run-off voting for their top choices based on their individual uptime_mempool performance stats. When the run-off voting is complete, the leaders are sorted in an array by average performance. The new leader list is hashed and broadcast to all nodes. The number of leaders depends on validation_task_mempool load for the previous 2 hours. A node that doesn't participate and broadcast as per these rules is disqualified to be a leader for 24 hours.


## Tech Stack

Mempools: RocksDB
Blockchain: XMBL Cubic DLT
Chrome Extension: Vue JS
Messaging: rust-libp2p

## Backend
Spawns a bunch of virtual nodes and leaders

## Extension
Real user interaction example. Hopefully some visuals for the cube constructions. 

## Simulator
Puts a tx load on the system to simulate real use.

## Installation & Usage

### Prerequisites
- Rust 1.70+ (with Cargo)
- Node.js 18+ (with npm)
- Chrome or Chromium browser (for extension)

### Backend Node (Rust Library)

The backend provides the core consensus library with all mempool operations, cryptographic functions, and consensus mechanisms.

```bash
# Navigate to backend directory
cd backend

# Build the library
cargo build --release

# Run all tests (204 tests covering the full system)
cargo test

# Run specific test modules
cargo test node_identity
cargo test mempool
cargo test transaction_workflow
cargo test leader_election
cargo test network_communication
cargo test integration

# Check code with clippy
cargo clippy
```

### Simulator (Rust CLI)

The simulator provides load testing, stress testing, and benchmarking capabilities for the consensus system.

```bash
# Navigate to simulator directory
cd simulator

# Build the simulator
cargo build --release

# Run basic load test (50 transactions, 10 nodes)
cargo run -- load-test

# Run stress test (1000 transactions, 100 nodes)
cargo run -- stress-test

# Run comprehensive benchmark
cargo run -- benchmark

# Custom load test with specific parameters
cargo run -- load-test --transactions 200 --nodes 20 --duration 30

# View help for all available options
cargo run -- --help
```

#### Simulator Commands

- `load-test`: Basic load testing with configurable parameters
- `stress-test`: High-load stress testing to find system limits
- `benchmark`: Performance benchmarking with detailed metrics
- `--transactions N`: Number of transactions to simulate
- `--nodes N`: Number of virtual nodes to spawn
- `--duration N`: Test duration in seconds

### Extension (Vue.js Dashboard)

The extension provides a real-time dashboard for monitoring the consensus system, viewing transactions, and managing nodes.

```bash
# Navigate to extension directory
cd extension

# Install dependencies
npm install

# Start development server
npm run serve

# Build for production
npm run build

# Run linter
npm run lint
```

#### Installing as Chrome Extension

1. Build the extension:
   ```bash
   cd extension
   npm run build
   ```

2. Open Chrome and navigate to `chrome://extensions/`

3. Enable "Developer mode" (toggle in top right)

4. Click "Load unpacked" and select the `extension/dist` directory

5. The PCL Dashboard extension will now appear in your browser toolbar

#### Extension Features

- **Real-time Dashboard**: Monitor system health, transaction throughput, and consensus state
- **Transaction Monitoring**: View active transactions across all mempools
- **Node Management**: Monitor node status, uptime, and performance metrics
- **Mempool Visualization**: Real-time view of all 5 mempools plus uptime tracking
- **Leader Election Tracking**: Monitor leader election cycles and voting rounds
- **Performance Metrics**: Charts showing system performance over time

### Development Workflow

1. **Start Backend Development**:
   ```bash
   cd backend
   cargo test --watch  # Run tests in watch mode
   ```

2. **Start Simulator for Testing**:
   ```bash
   cd simulator
   cargo run -- load-test --transactions 10 --nodes 5
   ```

3. **Start Extension Development**:
   ```bash
   cd extension
   npm run serve  # Hot-reload development server
   ```

4. **Run Full System Integration**:
   ```bash
   # Terminal 1: Backend tests
   cd backend && cargo test
   
   # Terminal 2: Simulator stress test
   cd simulator && cargo run -- stress-test
   
   # Terminal 3: Extension development
   cd extension && npm run serve
   ```

### System Architecture

The system consists of three main components:

1. **Backend Library** (`backend/`): Core consensus logic, cryptography, and mempool management
2. **Simulator** (`simulator/`): Load testing and performance benchmarking tools
3. **Extension** (`extension/`): Vue.js dashboard for monitoring and interaction

### Testing

The system includes comprehensive test coverage:

- **204 total tests** across 6 test modules
- **Unit tests** for individual components
- **Integration tests** for full system workflows
- **Load tests** via simulator for performance validation
- **End-to-end tests** for complete transaction workflows

Run the full test suite:
```bash
# Backend tests
cd backend && cargo test

# Simulator functionality
cd simulator && cargo run -- benchmark

# Extension tests (if you add them)
cd extension && npm test
```

### Performance Monitoring

Use the simulator to monitor system performance:

```bash
# Monitor transaction throughput
cargo run -- load-test --transactions 1000 --nodes 50

# Check consensus performance under load
cargo run -- stress-test

# Generate performance reports
cargo run -- benchmark > performance_report.txt
```

The extension dashboard provides real-time monitoring of:
- Transaction processing rates
- Mempool sizes and states
- Node uptime and response times
- Leader election cycles
- System health metrics

