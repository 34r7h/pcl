# Peer Consensus Layer - Application Interaction Specification

This document outlines the specification for how a client application, such as a Chrome Extension, interacts with the Peer Consensus Layer Rust backend node.

## 1. Architecture Overview

It is assumed that:
1.  Each user running the Chrome Extension will also be running an instance of the `peer_consensus_node` locally or have it configured to connect to a trusted personal node.
2.  The Chrome Extension communicates with its local/configured Rust node primarily via an HTTP API exposed by the Rust node.
3.  The Rust node handles all peer-to-peer networking, consensus logic, and mempool management.
4.  User identity (private keys for signing) is managed securely by the extension or a connected wallet, and signatures are provided to the node when necessary. The node itself should not store or request raw private keys.

## 2. API Endpoints

The Rust node (`peer_consensus_node`) should expose the following HTTP API endpoints. All request and response bodies are in JSON format.

### 2.1. Node Status & Information

*   **`GET /status`**
    *   Description: Retrieves the current status of the node, including its Node ID (PeerId), current leader status, and basic network health.
    *   Response Body:
        ```json
        {
            "node_id": "string (PeerId)",
            "is_leader": "boolean",
            "current_leader_id": "string (PeerId, if known and not self)",
            "connected_peers": "number",
            "uptime_seconds": "number",
            "version": "string (Node version)"
        }
        ```

### 2.2. Transaction Management

*   **`POST /transactions/submit`**
    *   Description: Submits a new transaction to the network via this node. The node will act as the initial leader (like "Charlie" in the README example) for this transaction.
    *   Request Body (`TransactionData` from `data_structures.rs`, `sig` must be present):
        ```json
        {
            "to": { "address1": "amount1", ... },
            "from": { "utxo_id1": "amount_from_utxo1", ... },
            "user": "string (user_address, e.g., public key hash)",
            "sig": "string (hex-encoded signature of the transaction data by the user)",
            "stake": "number (amount staked for validation)",
            "fee": "number (transaction fee)"
        }
        ```
    *   Successful Response Body (202 Accepted):
        ```json
        {
            "message": "Transaction accepted for processing.",
            "raw_tx_id": "string (hash of the raw transaction)"
        }
        ```
    *   Error Responses:
        *   `400 Bad Request`: Invalid transaction data, missing fields, signature verification failed.
        *   `503 Service Unavailable`: Node is unable to process the request at this time (e.g., not connected to the network).

*   **`GET /transactions/status/{raw_tx_id_or_final_tx_id}`**
    *   Description: Checks the status of a previously submitted or known transaction.
    *   Response Body:
        ```json
        {
            "query_id": "string (the ID used in the path)",
            "status": "string (e.g., 'pending_validation', 'in_raw_mempool', 'processing', 'in_processing_mempool', 'finalized', 'failed', 'unknown')",
            "raw_tx_id": "string (if applicable)",
            "final_tx_id": "string (if applicable, once processed)",
            "details": { /* Object containing more specific details depending on status */
                "timestamp_received": "number (Unix millis, if known)",
                "leader_node_id": "string (initial leader, if known)",
                "validations_received": "number (if in validation stage)",
                "required_validations": "number (if in validation stage)",
                "failure_reason": "string (if status is 'failed')"
            }
        }
        ```

### 2.3. User Validation Tasks

The README describes users (like Alice) performing validation tasks. The mechanism for assigning and submitting these tasks needs an API.

*   **`GET /validation/tasks`**
    *   Description: Fetches pending validation tasks assigned to the user associated with this node/extension. The node needs to identify the user, perhaps via a pre-configured user public key or an authenticated session if more advanced auth is implemented. For simplicity, assume the node serves tasks for its primary configured user.
    *   Response Body:
        ```json
        {
            "tasks": [
                {
                    "raw_tx_id": "string (the raw transaction these tasks are for)",
                    "task_id": "string (unique ID for this specific task)",
                    "type": "string (e.g., 'validate_signature_and_balance', 'verify_leader_calculations', 'dlt_specific_finality_check')",
                    "description": "string (human-readable description of the task)",
                    "data_required_for_validation": { /* object: data Alice needs to perform the task */ }
                }
                // ... more tasks
            ]
        }
        ```

*   **`POST /validation/submit_completed_task`**
    *   Description: Submits a completed validation task's results.
    *   Request Body:
        ```json
        {
            "raw_tx_id": "string",
            "task_id": "string",
            "completed_data": { /* object: results of the validation, e.g., a boolean success flag, signed data */ },
            "user_signature_on_completion": "string (hex-encoded signature of task_id and completed_data by the user)"
        }
        ```
    *   Successful Response Body (200 OK):
        ```json
        {
            "message": "Validation task submission received."
        }
        ```
    *   Error Responses:
        *   `400 Bad Request`: Invalid data, task ID not found or already completed.
        *   `404 Not Found`: Task ID does not correspond to an active task for this user.

### 2.4. User/Wallet Information (Optional - depends on design)

*   **`GET /user/balance`**
    *   Description: Retrieves the user's balance based on finalized UTXOs known to the node. This is a complex query and might be better handled by a dedicated indexer or explorer. If implemented, it would be a best-effort view from the node's perspective.
    *   Query Parameters: `user_address=string`
    *   Response Body:
        ```json
        {
            "user_address": "string",
            "available_utxos": [
                { "utxo_id": "string", "amount": "number", "tx_id_origin": "string" }
            ],
            "total_balance": "number"
        }
        ```

## 3. Data Formats

*   **TransactionData**: As defined in `peer_consensus_node/src/data_structures.rs` and used in `POST /transactions/submit`.
    ```rust
    // From data_structures.rs
    // #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    // pub struct TransactionData {
    //     pub to: HashMap<String, f64>, // address: amount
    //     pub from: HashMap<String, f64>, // utxo_id: amount
    //     pub user: String, // user_address
    //     pub sig: Option<String>, // Signature of the transaction data (all fields except sig itself)
    //     pub stake: f64,
    //     pub fee: f64,
    // }
    ```
*   **Signatures**: All signatures are expected to be hex-encoded strings of the raw signature bytes. The signing mechanism (e.g., Ed25519) should be consistent. The data to be signed for a transaction is the `TransactionData` object itself (e.g., canonical JSON representation) with the `sig` field absent or null during signing.
*   **Addresses/IDs**: User addresses, Node IDs (PeerIds), UTXO IDs, and Transaction IDs are typically represented as strings. Specific formats (e.g., base58 for PeerIds, hex for hashes) should be adhered to.

## 4. Workflow Examples from Chrome Extension

### 4.1. User Sends a Transaction

1.  **Extension UI**: User inputs transaction details (recipient, amount, etc.).
2.  **Extension Logic**:
    a.  Constructs the `TransactionData` object.
    b.  Calculates available UTXOs to cover the amount + fee + stake. Populates the `from` field.
    c.  Prompts the user to sign the transaction data (e.g., via an integrated wallet or by requesting a signature from a hardware wallet). The signature is over a canonical representation of the `TransactionData` (excluding the `sig` field).
    d.  Sets the `sig` field with the hex-encoded signature.
3.  **Extension API Call**: Makes a `POST /transactions/submit` request to the local Rust node with the signed `TransactionData`.
4.  **Rust Node**:
    a.  Receives the request.
    b.  Validates the `TransactionData` (schema, values).
    c.  Verifies the user's signature (`sig`) against the `user` public key and the rest of the transaction data.
    d.  If valid, accepts the transaction, creates a `RawTransactionEntry`, stores it in its `raw_tx_mempool`, locks UTXOs, and gossips it to other leaders (as per README Step 2).
    e.  Responds to the extension with the `raw_tx_id`.
5.  **Extension UI**: Shows a confirmation message with the `raw_tx_id` and indicates the transaction is processing. The extension can periodically poll `GET /transactions/status/{raw_tx_id}` for updates.

### 4.2. User Performs a Validation Task

1.  **Extension Logic**: Periodically calls `GET /validation/tasks` for the current user.
2.  **Extension UI**: If tasks are available, displays them to the user, providing necessary data and instructions.
3.  **User Action**: User reviews the task (e.g., confirms a signature verification, checks some data).
4.  **Extension Logic**:
    a.  Collects the user's input/result for the task.
    b.  Constructs the request body for `POST /validation/submit_completed_task`, including the `completed_data`.
    c.  Prompts the user to sign the `task_id` and `completed_data`.
    d.  Sets `user_signature_on_completion`.
5.  **Extension API Call**: Makes a `POST /validation/submit_completed_task` request.
6.  **Rust Node**:
    a.  Receives the submission.
    b.  Verifies the signature.
    c.  Finds the corresponding task in its internal state (e.g., within a `RawTransactionEntry`'s `validation_tasks`).
    d.  Processes the `completed_data`, updates the task status, and adds a validation timestamp.
    e.  If this completion leads to the transaction meeting requirements (e.g., sufficient validation timestamps), the node proceeds with further consensus steps (e.g., moving to `processing_tx_mempool`).
7.  **Extension UI**: Shows confirmation of task submission.

## 5. Security Considerations

*   **Signature Verification**: The Rust node MUST rigorously verify all incoming signatures (transaction signatures, validation task completion signatures) to ensure authenticity and integrity.
*   **Input Validation**: All API inputs must be strictly validated (data types, value ranges, string formats) to prevent injection attacks or crashes.
*   **HTTPS**: If the HTTP API is exposed to anything beyond `localhost` or a trusted local network, it MUST use HTTPS. For local-only interaction, HTTP might be acceptable but introduces risks if other processes on the machine can intercept local traffic.
*   **Authentication/Authorization (for future extension)**: The current spec assumes a single-user context per node. If a node were to serve multiple users or more sensitive operations, proper authentication (e.g., API keys, tokens) and authorization mechanisms would be needed for API endpoints.
*   **Rate Limiting**: Consider implementing rate limiting on API endpoints to prevent abuse or DoS attacks, especially for endpoints that trigger computationally intensive operations.
*   **Private Key Management**: The Chrome Extension (or its associated wallet component) is responsible for secure private key storage and signing operations. Private keys should never be sent to the Rust node.

## 6. Future Considerations

*   **WebSocket API**: For real-time updates (e.g., transaction status changes, new validation tasks), a WebSocket API could be more efficient than polling.
*   **GraphQL API**: For more flexible data querying, a GraphQL endpoint could be an alternative or addition to REST.
*   **Node Configuration API**: Endpoints to allow the extension to configure certain aspects of the node (if deemed safe and necessary).
*   **Multi-User Support**: If a single node instance is intended to support multiple distinct users via the extension, the API and internal logic would need significant changes to handle user separation, authentication, and task assignment.

This specification provides a baseline for interaction. The `peer_consensus_node` will need to implement an HTTP server (e.g., using `actix-web`, `axum`, or `warp`) to expose these endpoints and wire them to the existing consensus logic.
