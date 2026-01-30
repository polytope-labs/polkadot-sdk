# Revive RPC Server - Technical Documentation

## Overview

The `pallet-revive-eth-rpc` is an **Ethereum JSON-RPC compatibility layer** for the Polkadot SDK's `pallet-revive`. It acts as a bridge that allows Ethereum tooling (like MetaMask, Hardhat, Ethers.js) to interact with Substrate-based chains that use the Revive pallet.

This server translates Ethereum JSON-RPC calls into Substrate runtime API calls, manages transaction receipts, and provides full Ethereum compatibility while running on a Substrate blockchain.

---

## Architecture & Key Components

### 1. Core Server (`lib.rs` - `EthRpcServerImpl`)

The main RPC server implementation that handles all Ethereum JSON-RPC methods.

**Responsibilities:**
- Implements all standard Ethereum JSON-RPC methods (via the `EthRpcServer` trait)
- Translates Ethereum RPC calls into Substrate runtime API calls
- Manages accounts for signing transactions
- Handles both `eth_sendRawTransaction` and `eth_sendTransaction`
- Validates transactions (chain ID, signatures)
- Coordinates with client for blockchain interactions

**Key Methods Implemented:**
- `eth_accounts` - List managed accounts
- `eth_blockNumber` - Get current block number
- `eth_call` - Execute read-only contract calls
- `eth_sendRawTransaction` - Submit signed transactions
- `eth_sendTransaction` - Sign and submit transactions
- `eth_getBalance` - Query account balances
- `eth_getCode` - Get contract bytecode
- `eth_getTransactionReceipt` - Get transaction receipts
- `eth_getLogs` - Query event logs
- `eth_estimateGas` - Estimate gas costs
- `eth_gasPrice` - Get current gas price
- `eth_feeHistory` - Get fee history (EIP-1559)
- Plus many more standard Ethereum RPC methods

**Configuration:**
- `accounts`: List of accounts managed by the server for signing
- `allow_unprotected_txs`: Whether to accept transactions without chain ID

---

### 2. Client (`client.rs` - `Client`)

The core communication layer that manages all interactions with the Substrate node.

**Structure:**
```rust
pub struct Client {
    api: OnlineClient<SrcChainConfig>,
    rpc_client: RpcClient,
    rpc: LegacyRpcMethods<SrcChainConfig>,
    receipt_provider: ReceiptProvider,
    block_provider: SubxtBlockInfoProvider,
    fee_history_provider: FeeHistoryProvider,
    chain_id: u64,
    max_block_weight: Weight,
    automine: bool,
    block_notifier: Option<tokio::sync::broadcast::Sender<H256>>,
    subscription_lock: Arc<Mutex<()>>,
}
```

**Responsibilities:**
- **Connects to Substrate node** via WebSocket using `subxt` library
- **Manages runtime API calls** to query chain state and execute contract methods
- **Handles block subscriptions** for both best blocks and finalized blocks
- **Provides storage API** for accessing on-chain data
- **Coordinates between multiple providers** (block info, receipts, fee history)
- **Auto-reconnecting** RPC client with exponential backoff (100ms to 10s)
- **Automine support** for dev chains (waits for transactions to be included)

**Key Features:**
- Runtime API wrapper for type-safe calls to pallet-revive
- Storage API for direct chain state access
- Block subscription management with caching
- Transaction submission with optional block inclusion waiting
- System health monitoring
- Debug tracing support

**Connection Process:**
```rust
pub async fn connect(node_rpc_url: &str) -> Result<...> {
    // Creates reconnecting RPC client with exponential backoff
    // Initializes OnlineClient for typed API access
    // Sets up LegacyRpcMethods for raw RPC calls
}
```

---

### 3. Receipt Provider (`receipt_provider.rs` - `ReceiptProvider`)

Manages transaction receipts using a **SQLite database** with in-memory caching.

**Database Schema (3 tables):**

1. **`transaction_hashes`** - Maps Ethereum transaction hashes to Substrate blocks
   - `transaction_hash` (BLOB, PRIMARY KEY)
   - `transaction_index` (INTEGER)
   - `block_hash` (BLOB, indexed)

2. **`logs`** - Stores event logs with topic indexing for efficient filtering
   - `block_hash` (BLOB)
   - `transaction_index` (INTEGER)
   - `log_index` (INTEGER)
   - `address` (BLOB)
   - `block_number` (INTEGER)
   - `transaction_hash` (BLOB)
   - `topic_0`, `topic_1`, `topic_2`, `topic_3` (BLOB)
   - `data` (BLOB)
   - Composite PRIMARY KEY: (block_hash, transaction_index, log_index)
   - Multi-column index: (block_number, address, topic_0-3)

3. **`eth_to_substrate_blocks`** - Maps Ethereum block hashes to Substrate block hashes
   - `ethereum_block_hash` (BLOB, PRIMARY KEY)
   - `substrate_block_hash` (BLOB, indexed)

**Features:**
- **In-memory caching** with configurable retention (default: 256 blocks)
- **Archive node support** with persistent SQLite database
- **Memory-only mode** using `sqlite::memory:` for non-archive nodes
- **Automatic pruning** of old blocks when cache limit reached
- **Efficient log filtering** with indexed topics
- **Block reorganization handling** via fork detection

**Key Methods:**
- `insert_block_receipts()` - Store receipts for a block
- `receipt_by_hash()` - Query receipt by transaction hash
- `logs()` - Filter logs with topic and address matching
- `receipts_count_per_block()` - Count transactions in a block
- `get_substrate_hash()` / `get_ethereum_hash()` - Block hash translation

---

### 4. Block Info Provider (`block_info_provider.rs` - `SubxtBlockInfoProvider`)

Caches and retrieves block information with efficient lookups.

**Structure:**
```rust
pub struct SubxtBlockInfoProvider {
    latest_block: Arc<RwLock<Arc<SubstrateBlock>>>,
    latest_finalized_block: Arc<RwLock<Arc<SubstrateBlock>>>,
    rpc: LegacyRpcMethods<SrcChainConfig>,
    api: OnlineClient<SrcChainConfig>,
}
```

**Responsibilities:**
- **Tracks latest block** and **latest finalized block** in memory
- **Queries historical blocks** by number or hash via RPC
- **Caching strategy** - checks latest/finalized first, then queries RPC
- **Updates via subscriptions** when new blocks arrive

**Trait:**
```rust
pub trait BlockInfoProvider {
    async fn update_latest(&self, block, subscription_type);
    async fn latest_finalized_block(&self) -> Arc<SubstrateBlock>;
    async fn latest_block(&self) -> Arc<SubstrateBlock>;
    async fn block_by_number(&self, block_number) -> Result<Option<...>>;
    async fn block_by_hash(&self, hash) -> Result<Option<...>>;
}
```

---

### 5. Receipt Extractor (`receipt_extractor.rs` - `ReceiptExtractor`)

Extracts Ethereum-style transaction receipts from Substrate extrinsics.

**Responsibilities:**
- **Parses `EthTransact` extrinsics** from Substrate blocks
- **Recovers Ethereum addresses** from transaction signatures
- **Constructs Ethereum-compatible receipts** with all required fields
- **Handles contract deployment** address calculation (CREATE1)
- **Extracts logs** from `ContractEmitted` events
- **Calculates gas usage** from weight consumption
- **Detects reverts** from `EthExtrinsicRevert` events

**Key Components:**
```rust
pub struct ReceiptExtractor {
    fetch_receipt_data: FetchReceiptDataFn,
    fetch_eth_block_hash: FetchEthBlockHashFn,
    earliest_receipt_block: Option<SubstrateBlockNumber>,
    recover_eth_address: RecoverEthAddressFn,
}
```

**Receipt Construction:**
- Transaction hash: `keccak256(signed_transaction_bytes)`
- Contract address: Calculated using CREATE1 formula for deployments
- Logs: Extracted from pallet events, converted to Ethereum format
- Gas used: Derived from Substrate weight consumption
- Status: Success (1) or Failure (0) based on execution result

**Customization:**
Supports custom Ethereum address recovery logic if needed (though default is signature-based recovery).

---

### 6. Fee History Provider (`fee_history_provider.rs` - `FeeHistoryProvider`)

Implements `eth_feeHistory` for EIP-1559 gas price estimation support.

**Structure:**
```rust
#[derive(Default, Clone)]
struct FeeHistoryCacheItem {
    base_fee: u128,
    gas_used_ratio: f64,
    rewards: Vec<u128>,  // Priority fee percentiles
}

pub struct FeeHistoryProvider {
    fee_history_cache: Arc<RwLock<BTreeMap<SubstrateBlockNumber, FeeHistoryCacheItem>>>,
}
```

**Features:**
- **Caches up to 1024 blocks** of fee history
- **Calculates priority fee percentiles** (0.0 to 100.0 with 0.5 resolution = 200 points)
- **Tracks base fees** and **gas usage ratios**
- **Automatic eviction** of oldest entries when cache full

**Percentile Calculation:**
- Sorts transactions by effective reward (gas price - base fee)
- Calculates cumulative gas usage
- Maps percentiles to rewards based on gas consumption distribution

**Used By:**
- Wallets for gas price estimation
- EIP-1559 transaction construction
- Fee market analysis

---

### 7. APIs Module (`apis/`)

Defines multiple RPC API surfaces for different use cases.

#### **`execution_apis.rs` - `EthRpc` trait**
Standard Ethereum JSON-RPC methods (eth_*, net_*, web3_*)
- All methods listed in section 1 (Core Server)

#### **`debug_apis.rs` - `DebugRpc` trait**
Debug tracing APIs for transaction analysis:
- `debug_traceTransaction` - Trace a specific transaction
- `debug_traceBlockByNumber` - Trace all transactions in a block
- `debug_traceCall` - Trace a simulated call

#### **`health_api.rs` - `SystemHealthRpc` trait**
System health monitoring:
- `system_health` - Get node health status

#### **`polkadot_api.rs` - `PolkadotRpc` trait**
Polkadot-specific extensions (if any)

---

### 8. CLI (`cli.rs` - `CliCommand`)

Command-line interface for running the RPC server.

**Configuration Options:**
```rust
pub struct CliCommand {
    /// Node WebSocket URL (default: ws://127.0.0.1:9944)
    node_rpc_url: String,
    
    /// In-memory cache size in blocks (default: 256)
    cache_size: usize,
    
    /// Earliest block to search for receipts
    earliest_receipt_block: Option<SubstrateBlockNumber>,
    
    /// Database path (default: in-memory)
    /// Set to file path for archive node
    database_url: String,
    
    /// Index last N blocks on startup
    index_last_n_blocks: Option<SubstrateBlockNumber>,
    
    /// Allow unprotected transactions (no chain ID)
    allow_unprotected_txs: bool,
    
    /// Standard Substrate CLI params
    shared_params: SharedParams,
    rpc_params: RpcParams,
    prometheus_params: PrometheusParams,
}
```

**Default Ports:**
- RPC server: `8545` (standard Ethereum JSON-RPC port)
- Prometheus metrics: `9616`

**Dev Mode:**
When `--dev` flag is set:
- Pre-funds 5 test accounts: Alith, Baltathar, Charleth, Dorothy, Ethan
- Uses in-memory database
- Enables local CORS
- More verbose logging

**Startup Process:**
1. Initialize logger with specified filters
2. Connect to Substrate node
3. Initialize SQLite database (in-memory or persistent)
4. Create ReceiptExtractor with runtime API access
5. Create ReceiptProvider with database connection
6. Build Client with all providers
7. Optional: Index last N blocks in background
8. Start RPC server on configured address
9. Start Prometheus metrics endpoint
10. Subscribe to new blocks (both best and finalized)
11. Run until signal (SIGTERM/SIGINT)

---

## Data Flow

### Transaction Submission Flow

```
1. User/Wallet sends eth_sendRawTransaction
   ↓
2. EthRpcServerImpl receives signed Ethereum transaction bytes
   ↓
3. Server validates transaction:
   - Decodes TransactionSigned from RLP
   - Checks chain ID if unprotected txs not allowed
   - Verifies signature validity
   ↓
4. Calculate Ethereum transaction hash: keccak256(tx_bytes)
   ↓
5. Wrap in Substrate extrinsic: revive.eth_transact(tx_bytes)
   ↓
6. Client submits extrinsic to Substrate node via subxt
   ↓
7. If automine enabled:
   - Subscribe to block notifier
   - Wait for transaction inclusion (500ms timeout)
   - Check each new block for transaction hash
   ↓
8. Return Ethereum transaction hash to user
   ↓
9. Background: Block subscription detects new block
   ↓
10. ReceiptExtractor processes block:
    - Finds EthTransact extrinsics
    - Extracts receipts with logs and gas info
    - Calculates contract addresses for deployments
    ↓
11. ReceiptProvider stores in database:
    - Insert transaction hash mapping
    - Insert logs with indexed topics
    - Insert Ethereum ↔ Substrate block hash mapping
    ↓
12. User can query: eth_getTransactionReceipt(hash)
```

### Query Flow (eth_call, eth_getBalance, etc.)

```
1. User/Wallet sends eth_call or eth_getBalance
   ↓
2. EthRpcServerImpl receives request with:
   - Address/Transaction details
   - Block number/tag/hash (latest, earliest, pending, or specific)
   ↓
3. Client.block_hash_for_tag() resolves to Substrate block hash:
   - "latest" → latest_block().hash()
   - "earliest" → genesis block
   - "finalized" → latest_finalized_block().hash()
   - Number → query BlockInfoProvider
   - Ethereum hash → query ReceiptProvider for mapping
   ↓
4. Client.runtime_api(block_hash) creates RuntimeApi wrapper
   ↓
5. Call specific runtime API method:
   - balance(address) for eth_getBalance
   - code(address) for eth_getCode
   - dry_run(transaction) for eth_call or eth_estimateGas
   - nonce(address) for eth_getTransactionCount
   - get_storage(address, slot) for eth_getStorageAt
   ↓
6. Runtime executes at specific block height:
   - Reads state at that block
   - Executes EVM-compatible operation
   - Returns result in Substrate format
   ↓
7. Client converts to Ethereum format:
   - Balance: u128 → U256
   - Code: Vec<u8> → Bytes
   - Call result: decode output and gas
   ↓
8. EthRpcServerImpl returns to user
```

### Log Filtering Flow (eth_getLogs)

```
1. User sends eth_getLogs with Filter:
   - fromBlock, toBlock (range)
   - address (contract address or list)
   - topics (event signatures and indexed params)
   ↓
2. EthRpcServerImpl.get_logs() calls Client.logs()
   ↓
3. Client.logs() calls ReceiptProvider.logs()
   ↓
4. ReceiptProvider queries SQLite database:
   SQL: SELECT * FROM logs
   WHERE block_number >= ? AND block_number <= ?
   AND address IN (...)
   AND (topic_0 = ? OR topic_0 IS NULL)
   AND (topic_1 = ? OR topic_1 IS NULL)
   ... etc
   ↓
5. Database uses multi-column index for efficient lookup
   ↓
6. Results converted to Ethereum Log format:
   - address, topics[], data
   - blockNumber, blockHash, transactionHash
   - transactionIndex, logIndex
   ↓
7. Return FilterResults::Logs(Vec<Log>)
```

### Block Subscription Flow

```
1. CLI starts two background tasks:
   - subscribe_and_cache_new_blocks(BestBlocks)
   - subscribe_and_cache_new_blocks(FinalizedBlocks)
   ↓
2. Client subscribes to Substrate block events:
   - rpc.chain_subscribe_new_heads() for best blocks
   - rpc.chain_subscribe_finalized_heads() for finalized
   ↓
3. For each new block received:
   ↓
4. Fetch full block: api.blocks().at(block_hash)
   ↓
5. Update BlockInfoProvider cache:
   - block_provider.update_latest(block, subscription_type)
   ↓
6. Extract receipts: receipt_extractor.receipts_from_block()
   ↓
7. Store in database: receipt_provider.insert_block_receipts()
   ↓
8. Update fee history: fee_history_provider.update_fee_history()
   ↓
9. If automine: notify block_notifier subscribers
   ↓
10. Loop back to step 3
```

---

## Required Components

### Infrastructure Requirements

1. **Substrate Node with pallet-revive**
   - Must be running and accessible via WebSocket
   - Default endpoint: `ws://127.0.0.1:9944`
   - Should support subscriptions (newHead, finalizedHead)

2. **SQLite Database** (optional but recommended)
   - In-memory mode: `sqlite::memory:` (default)
   - Persistent mode: File path (e.g., `/data/eth_rpc.db`)
   - Archive nodes should use persistent database
   - Migrations run automatically on startup

3. **WebSocket Endpoint**
   - Must support Substrate RPC methods
   - Should be reliable (reconnection logic handles temporary failures)

### Runtime Requirements

The target Substrate chain **must have** `pallet-revive` with these runtime APIs:

#### **ReviveApi** trait methods:
- `dry_run(transaction, block)` - Simulate transaction execution
  - Returns: gas usage, return data, execution result
  
- `balance(address)` - Query account balance
  - Returns: u128 balance

- `nonce(address)` - Get transaction count for address
  - Returns: U256 nonce

- `code(address)` - Get contract bytecode
  - Returns: Vec<u8> code

- `gas_price()` - Get current gas price
  - Returns: U256 gas price

- `get_storage(address, key)` - Read contract storage
  - Returns: Option<[u8; 32]>

- `eth_block_hash(block_number)` - Get Ethereum block hash
  - Returns: Option<H256>

#### **Constants:**
- `pallet_revive::ChainId` - The EVM chain ID
- `frame_system::BlockWeights` - For gas calculation

#### **Events:**
- `pallet_revive::ContractEmitted` - Contract log events
- `pallet_revive::EthExtrinsicRevert` - Transaction reverts

### Configuration Options

```bash
# Minimal (in-memory, 256 block cache)
eth-rpc --node-rpc-url ws://localhost:9944

# Archive node (persistent DB)
eth-rpc \
  --node-rpc-url ws://localhost:9944 \
  --database-url sqlite:///data/eth_rpc.db \
  --cache-size 1000

# Index historical blocks on startup
eth-rpc \
  --node-rpc-url ws://localhost:9944 \
  --index-last-n-blocks 10000 \
  --earliest-receipt-block 1

# Dev mode with test accounts
eth-rpc --dev \
  --node-rpc-url ws://localhost:9944 \
  --allow-unprotected-txs

# Custom ports
eth-rpc \
  --node-rpc-url ws://localhost:9944 \
  --rpc-port 8545 \
  --prometheus-port 9616

# Full production setup
eth-rpc \
  --node-rpc-url wss://mainnet.example.com:443 \
  --database-url sqlite:///var/lib/eth-rpc/receipts.db \
  --cache-size 2048 \
  --rpc-port 8545 \
  --rpc-cors all \
  --prometheus-port 9616 \
  --earliest-receipt-block 1000000
```

### Rust Dependencies (from Cargo.toml)

**Core:**
- `jsonrpsee` - JSON-RPC server framework
- `subxt` - Substrate client library
- `tokio` - Async runtime
- `sqlx` - SQLite database with compile-time query checking

**Substrate:**
- `sc-cli`, `sc-service`, `sc-rpc` - Substrate client libraries
- `sp-core`, `sp-runtime`, `sp-weights` - Substrate primitives
- `pallet-revive` - The EVM compatibility pallet

**Utilities:**
- `serde`, `serde_json` - Serialization
- `codec` - SCALE encoding
- `rlp` - Ethereum RLP encoding
- `hex` - Hex encoding/decoding
- `log` - Logging framework
- `anyhow`, `thiserror` - Error handling

**Development:**
- `subxt-signer` - For dev account management
- `pallet-revive-fixtures` - Test contracts
- `revive-dev-node` - Local test node

---

## Key Features

### 1. Full Ethereum JSON-RPC Compatibility
Implements the complete Ethereum JSON-RPC specification, enabling seamless integration with:
- MetaMask and other Web3 wallets
- Hardhat, Truffle, Foundry development tools
- Ethers.js, Web3.js, Viem libraries
- Block explorers (with minor adaptations)

### 2. Transaction Signing
**Server-Side Signing:**
- Manages accounts with private keys
- Signs transactions via `eth_sendTransaction`
- Dev mode includes 5 pre-funded test accounts

**Client-Side Signing:**
- Accepts pre-signed transactions via `eth_sendRawTransaction`
- Validates signatures and chain IDs
- Optional support for unprotected (legacy) transactions

### 3. Event Filtering
**Efficient Log Queries:**
- Multi-column SQL index on (block_number, address, topics)
- Supports complex filter combinations
- Topic wildcards and alternatives
- Block range queries

**Example Filters:**
```json
{
  "fromBlock": "0x1",
  "toBlock": "latest",
  "address": "0x1234...",
  "topics": [
    "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",
    null,
    ["0x000...alice", "0x000...bob"]
  ]
}
```

### 4. Block Caching
**Two-Tier Caching:**
- **In-memory**: Latest and finalized blocks (instant access)
- **SQLite**: Configurable block history (256-2048 blocks typical)

**Benefits:**
- Fast queries for recent data
- Reduced RPC load on Substrate node
- Archive capability with persistent database

### 5. Archive Mode
**Persistent Storage:**
- All transaction receipts stored in SQLite
- Full event log history with indexed search
- Block hash mappings for Ethereum compatibility

**Use Cases:**
- Block explorers
- Analytics platforms
- Historical transaction queries
- Compliance and auditing

### 6. Dev Mode
**Pre-funded Test Accounts:**
```
Alith:     0xf24FF3a9CF04c71Dbc94D0b566f7A27B94566cac
Baltathar: 0x3Cd0A705a2DC65e5b1E1205896BaA2be8A07c6e0
Charleth:  0x798d4Ba9baf0064Ec19eB4F0a1a45785ae9D6DFc
Dorothy:   0x773539d4Ac0e786233D90A233654ccEE26a613D9
Ethan:     0xFf64d3F6efE2317EE2807d223a0Bdc4c0c49dfDB
```

**Features:**
- Automatically unlocked for `eth_sendTransaction`
- Large initial balances
- Simplified testing workflow
- No need for external wallet setup

### 7. Metrics & Monitoring
**Prometheus Endpoint:**
- RPC call counts and latencies
- Database query performance
- Block processing metrics
- Error rates and types

**Health API:**
- Node sync status
- Peer count
- Chain info

### 8. Auto-Reconnection
**Resilient Connection:**
- Exponential backoff: 100ms → 10s
- Automatic reconnection on failure
- Continues serving cached data during reconnection
- Logs connection state changes

### 9. EIP-1559 Support
**Fee History:**
- Base fee tracking per block
- Priority fee percentile calculation
- Gas usage ratio history
- Supports up to 1024 blocks lookback

**Gas Price Estimation:**
- `eth_gasPrice` - Current base fee
- `eth_maxPriorityFeePerGas` - Returns 0 (no priority fees in Substrate)
- `eth_feeHistory` - Historical fee data for wallet estimation

### 10. Transaction Validation
**Protection Options:**
- Chain ID validation (EIP-155)
- Signature verification
- Gas limit checks
- Nonce management

**Configurable:**
- `--allow-unprotected-txs` for legacy transaction support
- Useful for testing and migration scenarios

### 11. Debug Tracing
**Transaction Analysis:**
- `debug_traceTransaction` - Step-by-step execution trace
- `debug_traceBlockByNumber` - Trace all transactions in a block
- `debug_traceCall` - Simulate and trace calls

**Tracer Types:**
- Call tracer (call tree with inputs/outputs)
- Opcode tracer (EVM instruction-level)
- Custom tracers (extensible)

### 12. Block Reorganization Handling
**Fork Detection:**
- Tracks block ancestry
- Detects chain reorganizations
- Updates database mappings
- Invalidates stale receipts

---

## Implementation Details

### Ethereum to Substrate Translation

#### **Transaction Hashes:**
```rust
// Ethereum transaction hash
eth_tx_hash = keccak256(rlp_encoded_signed_transaction)

// Substrate extrinsic hash (different!)
substrate_tx_hash = blake2_256(scale_encoded_extrinsic)

// We use and expose the Ethereum hash to users
```

#### **Block Hashes:**
```rust
// Ethereum block hash (calculated from block header)
eth_block_hash = calculated_from_evm_block_header()

// Substrate block hash
substrate_block_hash = blake2_256(block_header)

// We maintain mapping in database for translation
```

#### **Addresses:**
- Ethereum addresses (20 bytes) map to contract accounts in pallet-revive
- Account derivation uses Ethereum's addressing scheme
- CREATE1: `address = keccak256(rlp([sender, nonce]))[12:]`
- CREATE2: `address = keccak256(0xff ++ sender ++ salt ++ keccak256(code))[12:]`

#### **Gas Calculations:**
```rust
// Substrate uses Weight (ref_time, proof_size)
// Converted to Ethereum gas for display

eth_gas = weight.ref_time / gas_to_weight_ratio
base_fee = calculated_from_weight_prices
```

### Error Handling

**Error Types:**
```rust
pub enum EthRpcError {
    ClientError(ClientError),
    RlpError(rlp::DecoderError),
    ConversionError,
    InvalidSignature,
    AccountNotFound(H160),
    InvalidTransaction,
    TransactionTypeNotSupported(Byte),
}

pub enum ClientError {
    Jsonrpsee(jsonrpsee::Error),
    SubxtError(subxt::Error),
    SqlxError(sqlx::Error),
    BlockNotFound(H256),
    ContractNotFound(H160),
    TxDecodingFailed,
    // ... more variants
}
```

**Error Code Mapping:**
- Maps to Ethereum JSON-RPC error codes (EIP-1474)
- Preserves error messages from runtime
- Includes revert reasons for failed transactions

### Performance Considerations

**Database Indexes:**
```sql
-- Fast transaction lookup by hash
CREATE INDEX idx_transaction_hash ON transaction_hashes(transaction_hash);

-- Fast block-based queries
CREATE INDEX idx_block_hash ON transaction_hashes(block_hash);

-- Efficient log filtering
CREATE INDEX idx_block_number_address_topics ON logs(
    block_number, address, topic_0, topic_1, topic_2, topic_3
);
```

**Caching Strategy:**
- Latest block: Always in memory (RwLock)
- Recent blocks: Configurable LRU (256-2048 blocks)
- Historical: Database queries with indexes

**Concurrency:**
- Read-heavy workload optimized with RwLock
- Write operations (block insertion) serialized with Mutex
- Parallel RPC request handling via tokio

### Security Considerations

**Transaction Validation:**
- Signature verification before submission
- Chain ID enforcement (unless explicitly disabled)
- Gas limit bounds checking
- Nonce verification

**Database Security:**
- SQL injection protection via parameterized queries (sqlx)
- No user-controlled SQL construction
- File permissions on SQLite database

**RPC Security:**
- CORS configuration for browser access
- Rate limiting support (via CLI flags)
- IP whitelisting for rate limit bypass
- Request size limits

**Private Key Management:**
- Dev mode only - never use in production
- Production: Use `eth_sendRawTransaction` with external signers
- No key storage in database

---

## Testing

### Integration Tests
Located in `src/tests.rs`:
- End-to-end RPC method testing
- Receipt extraction and storage
- Log filtering with complex queries
- Block caching and pruning
- Fork handling

### Database Tests
Located in `src/receipt_provider.rs#tests`:
- Insert and remove operations
- Pruning logic
- Block mapping (Ethereum ↔ Substrate)
- Log query optimization

### Development Testing
```bash
# Run local dev node
revive-dev-node --dev

# In another terminal, run RPC server
cargo run --bin eth-rpc -- --dev

# Test with curl
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'

# Test with MetaMask
# Network: http://localhost:8545
# Chain ID: (get from eth_chainId)

# Run integration tests
cargo test
```

---

## Future Enhancements

Potential areas for improvement:

1. **Performance:**
   - PostgreSQL support for larger scale deployments
   - Read replicas for database
   - More aggressive caching strategies
   - Batch RPC request optimization

2. **Features:**
   - GraphQL API support
   - WebSocket subscriptions (eth_subscribe)
   - Transaction pool status queries
   - Block