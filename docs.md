```markdown
# Consensus Node Documentation

## 1. Overview

The `consensus_node` is a Rust application that implements a peer in a decentralized consensus layer. It participates in leader election, transaction processing, and validation according to a defined workflow. The system uses libp2p for peer-to-peer networking (including mDNS for discovery and gossipsub for message passing) and RocksDB for persistent storage of mempool data.

This document describes how to build, run, and understand the basic operation of the `consensus_node`.

## 2. Getting Started

### Prerequisites

*   **Rust:** Ensure you have a recent version of Rust and Cargo installed. You can get it from [rustup.rs](https://rustup.rs/).
*   **Build Essentials:** On some systems, you might need C++ compilers and development libraries for RocksDB to build correctly (e.g., `build-essential`, `clang`, `llvm`, `libgflags-dev` on Debian/Ubuntu).

### Building the Node

1.  Navigate to the `consensus_node` directory (the one containing `Cargo.toml`).
2.  Build the project using Cargo:
    *   For a debug build:
        ```bash
        cargo build
        ```
    *   For an optimized release build:
        ```bash
        cargo build --release
        ```
    The executable will be located in `target/debug/consensus_node` or `target/release/consensus_node`.

### Running a Node

1.  After building, you can run the node using Cargo:
    ```bash
    cargo run
    ```
    Or, if you built a release version:
    ```bash
    cargo run --release
    ```
    Alternatively, you can run the executable directly:
    ```bash
    ./target/debug/consensus_node
    # or for release
    ./target/release/consensus_node
    ```

2.  **Database:** Each node instance creates its own unique database directory in the current working directory where it's launched. The directory is named `db_<first_8_chars_of_public_key_hex>`, for example, `db_a1b2c3d4`. This allows multiple nodes to run locally without data conflicts.

3.  On startup, the node will print its libp2p PeerId and its application-level public key (Node Identity). It will also log the network address it's listening on.

## 3. Node Identity

Each `consensus_node` generates a unique Ed25519 keypair upon its first startup (if no existing identity is configured to be loaded, which is the current behavior).
*   The **public key** (represented as a hex string) serves as the node's primary application-level identifier. This ID is used in P2P messages, leader election, and transaction attributions.
*   The **private key** is used for signing messages, transactions, and votes, proving ownership of the public key.

The node's application public key hex is printed to the console on startup.

## 4. Networking (libp2p)

The node uses the `libp2p` framework for all peer-to-peer communications.

*   **Peer Discovery:**
    *   **mDNS:** For local network discovery, nodes use mDNS to find other peers. Discovered peers are automatically added to the gossipsub mesh.
*   **Communication Protocol:**
    *   **Gossipsub:** Most messages are broadcast over a common topic named `consensus-messages` using the gossipsub protocol. This is used for things like transaction dissemination, leader election messages, and uptime data.
*   **Listening Address:**
    *   By default, the node listens on all available network interfaces (`0.0.0.0`) on a system-assigned TCP port. The specific address and port are logged on startup (e.g., `/ip4/0.0.0.0/tcp/12345`).

## 5. Core Processes & Data Flow

The `consensus_node` implements several key processes as described in the main project `README.md`.

### 5.1. Leader Election

Nodes participate in a leader election process to determine which peers will take on leader responsibilities.
*   **Uptime & Performance:** Nodes (conceptually) send pulses and track uptime and response times for other nodes. This data is stored in their local `uptime_mempool`.
*   **Broadcasting Data:** Periodically (every `UPTIME_BROADCAST_INTERVAL_SECS`, default 300s), nodes broadcast their observed uptime data.
*   **Nomination & Voting:** Based on aggregated uptime data, nodes nominate candidates. This is followed by multiple rounds of voting (`NUM_VOTING_ROUNDS`, default 3) to select the top `NUM_LEADERS_TO_ELECT` (default 5) leaders.
*   **Leader List:** A new leader list is hashed and broadcast. Leaders are responsible for certain stages of transaction processing.

### 5.2. Transaction Workflow

The node processes transactions through several stages, involving different roles (User, Originating Leader "Charlie", Other Leaders "L2, L8", Validators).

1.  **Transaction Submission (Conceptual - `TxData`)**
    *   A user (e.g., Alice) creates a transaction (`TxData`) specifying recipients, amounts from her UTXOs, stake, and fee. She signs this transaction.
    *   *Note: The current `consensus_node` does not have an external API for users to submit transactions. The `p2p.rs` includes a test that simulates a transaction originating from the node itself if it's a leader.*

2.  **Raw Transaction Handling (Originating Leader "Charlie" - Steps 1 & 2 of README)**
    *   A leader node (Charlie) receives/creates the `TxData`.
    *   It calculates a `raw_tx_id` (hash of `TxData`).
    *   It creates a `RawTxMempoolEntry` and stores it in its RocksDB (`raw_tx_mempool`).
    *   UTXOs specified in `tx_data.from` are locked in the `locked_utxo_mempool`.
    *   The `raw_tx_id` is added to a list of transactions needing validation (`validation_tasks_mempool` entry).
    *   Charlie gossips the `RawTxMempoolEntry` to other leaders.
    *   Other leaders receiving the gossip also store the entry and lock UTXOs.

3.  **Validation Task Generation & Assignment (Other Leaders & Charlie - Step 3)**
    *   Other leaders (e.g., L2) see the gossiped `RawTxMempoolEntry`.
    *   They generate `ValidationTask`s for the user (Alice), typically `UserSignatureAndBalanceValidation`.
    *   These tasks are sent to Charlie via `OfferValidationTaskToOriginLeader` P2P messages.
    *   Charlie collects these offered tasks. When enough/appropriate tasks are collected (currently after the first offer in the code), Charlie officially assigns them to Alice by updating his `RawTxMempoolEntry` and (conceptually) notifying Alice via `ValidationTaskAssignmentToUser`.

4.  **User Task Completion (Alice -> L2 -> Charlie - Step 4)**
    *   Alice completes her assigned validation tasks.
    *   She sends a `UserValidationTaskCompletion` message (containing her signature on task details) to the leader who *generated* the task (e.g., L2).
    *   L2 verifies Alice's completion. If valid, L2 sends a `ForwardUserTaskCompletionToOriginLeader` message to Charlie.
    *   Charlie receives this, verifies the forwarded completion against the task in his `RawTxMempoolEntry`, marks the task as complete, and records Alice's validation timestamp.
    *   Once all user-specific tasks for a `raw_tx_id` are complete, Charlie removes the `raw_tx_id` from the general `validation_tasks_mempool`.

5.  **Processing Raw Transaction (Charlie - Step 5)**
    *   When all required validation tasks for Alice's transaction are complete and enough validation timestamps (`MIN_VALIDATION_TIMESTAMPS_FOR_PROCESSING`, default 1) are collected, Charlie proceeds.
    *   He averages the `validation_timestamps`.
    *   He creates a `ProcessingTxMempoolEntry` containing the original `TxData`, the averaged timestamp, and his (Charlie's) signature over these. The hash of `{averaged_timestamp + tx_data_hash}` becomes the `tx_id`.
    *   This entry is stored in the `processing_tx_mempool`. The original `RawTxMempoolEntry` is removed.
    *   A new validation task of type `LeaderTimestampMathCheck` is created for this `tx_id` and added to the `validation_tasks_mempool` for network validators.

6.  **Validator Math Check & Finality (Validator & Leaders - Step 6)**
    *   A node (acting as a validator, simulated in current code) picks up the `LeaderTimestampMathCheck` task.
    *   It verifies Charlie's math for the `ProcessingTxMempoolEntry` (recalculates `tx_id`, checks signature).
    *   If valid, the validator broadcasts a `VerifiedProcessingTxBroadcast` message to a few random leaders.
    *   Leaders receiving this message:
        *   Verify the information.
        *   Perform the DLT-specific finality task: calculate the digital root of the `tx_id` (as per XMBL Cubic DLT example in README).
        *   Store an entry in the `final_tx_mempool` (e.g., `{tx_id, digital_root, original_tx_data}`).
        *   Clean up the `RawTxMempoolEntry` (if any remnants), and the `validation_tasks_mempool` entries for both the original `raw_tx_id` and the `processing_tx_id`.
        *   Gossip the verified `ProcessingTxMempoolEntry` to all other leaders to ensure eventual consistency. Other leaders receiving this gossip also perform the finality steps and cleanup.

### 5.3. Transaction Invalidation

*   If a transaction is found to be invalid at any step (e.g., signature failure, double spend attempt, failed validation task), a `TransactionInvalidationNotice` P2P message can be broadcast.
*   Nodes receiving this notice will clean up all associated data for the specified `tx_id` (raw or processed) from their mempools (raw, processing, final, locked UTXOs, validation tasks).

## 6. Mempools (RocksDB Storage)

The `consensus_node` uses RocksDB for persistent storage of its mempools and other important state. Data is stored in a subdirectory named `db_<node_id_prefix>` in the working directory. Different types of data are organized using key prefixes:

*   **`rawtx_` (`DB_RAW_TX_MEMPOOL_PREFIX`):** Stores `RawTxMempoolEntry` objects, keyed by `raw_tx_id`.
*   **`valtask_` (`DB_VALIDATION_TASKS_MEMPOOL_PREFIX`):**
    *   For raw transactions: Key is `raw_tx_id`, value is a timestamp indicating it needs validation tasks.
    *   For processed transactions: Key is `processing_tx_id`, value is the `ValidationTask` details for validators (e.g., `LeaderTimestampMathCheck`).
*   **`lockutxo_` (`DB_LOCKED_UTXO_MEMPOOL_PREFIX`):** Key is `utxo_id`, value is the `raw_tx_id` that locked it. Prevents double spending.
*   **`proctx_` (`DB_PROCESSING_TX_MEMPOOL_PREFIX`):** Stores `ProcessingTxMempoolEntry` objects, keyed by `processing_tx_id`.
*   **`finaltx_` (`DB_FINAL_TX_MEMPOOL_PREFIX`):** Stores final transaction information after DLT-specific processing (e.g., including the digital root), keyed by `processing_tx_id`.
*   **`uptime_` (`DB_UPTIME_PREFIX`):** Stores `UptimeMempoolEntry` objects for observed peers, keyed by their node public key hex.

## 7. Key Data Structures

These are some of the primary Rust structs defined in `consensus_node/src/data_structures.rs` that are fundamental to the system's operation:

*   **`NodeIdentity`**: Holds a node's Ed25519 keypair and public key hex.
*   **`TxData`**: Represents a user's transaction, including inputs, outputs, stake, fee, and user signature.
*   **`ValidationTaskType`**: Enum for different types of validation tasks (e.g., `UserSignatureAndBalanceValidation`, `LeaderTimestampMathCheck`).
*   **`ValidationTask`**: Details of a validation task, including its type, relevant transaction ID(s), assigned/generating leader, completion status, and completion signature.
*   **`RawTxMempoolEntry`**: Wrapper for `TxData` in its initial mempool state, including associated validation tasks and timestamps.
*   **`ProcessingTxMempoolEntry`**: Represents a transaction after initial leader processing, including the averaged validation timestamp and leader's signature.
*   **`UptimeMempoolEntry`**: Tracks pulse data for leader election.
*   **`P2PMessage`**: Enum defining all types of messages exchanged over the network.

## 8. P2P Messages (`P2PMessage` Enum)

Nodes communicate using various message types defined in the `P2PMessage` enum. Key examples include:

*   `Pulse`, `PulseResponse`: For uptime tracking (conceptual).
*   `UptimeDataBroadcast`: Nodes share their observed uptime data for leader election.
*   `LeaderNominations`, `LeaderElectionVoteMsg`, `NewLeaderList`: Used during the leader election process.
*   `RawTransactionGossip`: For disseminating new raw transactions.
*   `OfferValidationTaskToOriginLeader`: Leaders offer tasks for a user to the transaction's originating leader.
*   `ValidationTaskAssignmentToUser`: Originating leader assigns tasks to a user (conceptual notification).
*   `UserValidationTaskCompletion`: User reports completed task to the task-generating leader.
*   `ForwardUserTaskCompletionToOriginLeader`: Task-generating leader forwards user's completion to originating leader.
*   `VerifiedProcessingTxBroadcast`: A validator broadcasts a processing transaction entry it has verified.
*   `ProcessingTransactionGossip`: Leaders disseminate processed transaction entries.
*   `TransactionInvalidationNotice`: To notify peers about an invalid transaction for cleanup.

## 9. Logging

The `consensus_node` application prints informational messages and errors to the standard output. This includes:
*   Local Peer ID and listening addresses.
*   mDNS peer discovery events.
*   Gossipsub message handling.
*   Connection establishments and closures.
*   Key steps in transaction processing and leader election.
*   Errors encountered during operation.

Look at the console output when running a node to observe its activity.

## 10. Configuration

Currently, most operational parameters are hardcoded as constants in `consensus_node/src/p2p.rs`. These include:
*   `NUM_LEADERS_TO_ELECT` (default: 5)
*   `NUM_VOTING_ROUNDS` (default: 3)
*   `UPTIME_BROADCAST_INTERVAL_SECS` (default: 300 seconds)
*   `ELECTION_PHASE_TIMEOUT_SECS` (default: 60 seconds)
*   `MIN_VALIDATION_TIMESTAMPS_FOR_PROCESSING` (default: 1)
*   `NUM_LEADERS_FOR_VALIDATOR_BROADCAST` (default: 3)

To change these, you would currently need to modify the source code and recompile.

---

This documentation provides a snapshot based on the current codebase. As the project evolves, this document should be updated.
```
