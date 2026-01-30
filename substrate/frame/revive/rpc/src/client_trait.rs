// This file is part of Substrate.

// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//! Trait abstraction for the Revive RPC client.
//!
//! This module provides a trait-based abstraction over the client functionality,
//! allowing for easier testing, mocking, and potential alternative implementations
//! in the future.
//!
//! # Architecture
//!
//! The `ClientT` trait abstracts all interactions with the Substrate chain and provides
//! Ethereum-compatible RPC methods. This enables:
//!
//! - **Testability**: Easy mocking for unit tests
//! - **Flexibility**: Support for both native Substrate clients and potential future implementations
//! - **Decoupling**: RPC server implementations depend on the trait, not concrete types
//!
//! # Implementation
//!
//! The primary implementation is the concrete `Client` struct which uses subxt for
//! Substrate chain interaction. Alternative implementations could include:
//! - Mock clients for testing
//! - Native node clients (future Phase 5 goal)
//! - Caching/proxy clients
//!
//! # Example
//!
//! ```ignore
//! use pallet_revive_eth_rpc::{client::Client, client_trait::ClientT};
//!
//! async fn process_blocks<C: ClientT>(client: &C) {
//!     let latest_block = client.latest_block().await;
//!     let block_number = latest_block.number();
//!     println!("Latest block: {}", block_number);
//! }
//! ```

use crate::{
	client::{
		runtime_api::RuntimeApi, storage_api::StorageApi, ClientError, SubstrateBlock,
		SubstrateBlockHash, SubstrateBlockNumber, SubscriptionType,
	},
	subxt_client::revive::calls::types::EthTransact,
	TracerType,
};
use pallet_revive::evm::{
	Block, BlockNumberOrTag, BlockNumberOrTagOrHash, FeeHistoryResult, Filter, Log, ReceiptInfo,
	SyncingStatus, Trace, TransactionSigned, TransactionTrace, H256,
};
use sp_weights::Weight;
use std::sync::Arc;
use subxt::backend::legacy::rpc_methods::SystemHealth;
use jsonrpsee::core::async_trait;

/// A trait abstracting the client functionality for interacting with the Substrate chain
/// and providing Ethereum-compatible RPC methods.
///
/// This trait defines the interface for all client operations needed by the Revive
/// Ethereum-compatible RPC server. It abstracts over:
///
/// - Block queries and subscriptions
/// - Transaction submission and receipt retrieval
/// - Runtime API access
/// - Storage queries
/// - Tracing and debugging
/// - Fee history and gas estimation
///
/// # Design Principles
///
/// - **Async-first**: All I/O operations are async for efficient concurrency
/// - **Error handling**: Methods return `Result<T, ClientError>` for fallible operations
/// - **Generic blocks**: Uses `Arc<SubstrateBlock>` for efficient sharing
/// - **Type safety**: Strongly typed parameters prevent runtime errors
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` to support concurrent access from
/// multiple RPC handlers.
#[async_trait]
pub trait ClientT: Send + Sync + 'static {
	/// Creates a block notifier instance.
	fn create_block_notifier(&mut self);

	/// Sets a block notifier.
	fn set_block_notifier(&mut self, notifier: Option<tokio::sync::broadcast::Sender<H256>>);

	/// Start the block subscription, and populate the block cache.
	async fn subscribe_and_cache_new_blocks(
		&self,
		subscription_type: SubscriptionType,
	) -> Result<(), ClientError>;

	/// Cache old blocks up to the given block number.
	async fn subscribe_and_cache_blocks(
		&self,
		index_last_n_blocks: SubstrateBlockNumber,
	) -> Result<(), ClientError>;

	/// Get the block hash for the given block number or tag.
	async fn block_hash_for_tag(
		&self,
		at: BlockNumberOrTagOrHash,
	) -> Result<SubstrateBlockHash, ClientError>;

	/// Get the storage API for the given block.
	fn storage_api(&self, block_hash: H256) -> StorageApi;

	/// Get the runtime API for the given block.
	fn runtime_api(&self, block_hash: H256) -> RuntimeApi;

	/// Get the latest finalized block.
	async fn latest_finalized_block(&self) -> Arc<SubstrateBlock>;

	/// Get the latest best block.
	async fn latest_block(&self) -> Arc<SubstrateBlock>;

	/// Submit a transaction to the chain.
	async fn submit(
		&self,
		call: subxt::tx::DefaultPayload<EthTransact>,
	) -> Result<(), ClientError>;

	/// Get an EVM transaction receipt by hash.
	async fn receipt(&self, tx_hash: &H256) -> Option<ReceiptInfo>;

	/// Get the post dispatch weight associated with this Ethereum transaction hash.
	async fn post_dispatch_weight(&self, tx_hash: &H256) -> Option<Weight>;

	/// Get the sync state of the chain.
	async fn sync_state(
		&self,
	) -> Result<sc_rpc::system::SyncState<SubstrateBlockNumber>, ClientError>;

	/// Get the syncing status of the chain.
	async fn syncing(&self) -> Result<SyncingStatus, ClientError>;

	/// Get an EVM transaction receipt by block hash and transaction index.
	async fn receipt_by_hash_and_index(
		&self,
		block_hash: &H256,
		transaction_index: usize,
	) -> Option<ReceiptInfo>;

	/// Get a signed transaction by hash.
	async fn signed_tx_by_hash(&self, tx_hash: &H256) -> Option<TransactionSigned>;

	/// Get receipts count per block.
	async fn receipts_count_per_block(&self, block_hash: &SubstrateBlockHash) -> Option<usize>;

	/// Get an EVM transaction receipt by specified Ethereum block hash and index.
	async fn receipt_by_ethereum_hash_and_index(
		&self,
		ethereum_hash: &H256,
		transaction_index: usize,
	) -> Option<ReceiptInfo>;

	/// Get the system health.
	async fn system_health(&self) -> Result<SystemHealth, ClientError>;

	/// Get the block number of the latest block.
	async fn block_number(&self) -> Result<SubstrateBlockNumber, ClientError>;

	/// Get a block hash for the given block number.
	async fn get_block_hash(
		&self,
		block_number: SubstrateBlockNumber,
	) -> Result<Option<SubstrateBlockHash>, ClientError>;

	/// Get a block for the specified hash or number.
	async fn block_by_number_or_tag(
		&self,
		block: &BlockNumberOrTag,
	) -> Result<Option<Arc<SubstrateBlock>>, ClientError>;

	/// Get a block by hash.
	async fn block_by_hash(
		&self,
		hash: &SubstrateBlockHash,
	) -> Result<Option<Arc<SubstrateBlock>>, ClientError>;

	/// Resolve Ethereum block hash to Substrate block hash.
	async fn resolve_substrate_hash(&self, ethereum_hash: &H256) -> Option<H256>;

	/// Resolve Substrate block hash to Ethereum block hash.
	async fn resolve_ethereum_hash(&self, substrate_hash: &H256) -> Option<H256>;

	/// Get a block by Ethereum hash with automatic resolution to Substrate hash.
	async fn block_by_ethereum_hash(
		&self,
		ethereum_hash: &H256,
	) -> Result<Option<Arc<SubstrateBlock>>, ClientError>;

	/// Get a block by number.
	async fn block_by_number(
		&self,
		block_number: SubstrateBlockNumber,
	) -> Result<Option<Arc<SubstrateBlock>>, ClientError>;

	/// Get the transaction traces for the given block.
	async fn trace_block_by_number(
		&self,
		at: BlockNumberOrTag,
		config: TracerType,
	) -> Result<Vec<TransactionTrace>, ClientError>;

	/// Get the transaction traces for the given transaction.
	async fn trace_transaction(
		&self,
		transaction_hash: H256,
		config: TracerType,
	) -> Result<Trace, ClientError>;

	/// Get the transaction traces for the given call.
	async fn trace_call(
		&self,
		transaction: pallet_revive::evm::GenericTransaction,
		block: BlockNumberOrTagOrHash,
		config: TracerType,
	) -> Result<Trace, ClientError>;

	/// Get the EVM block for the given Substrate block.
	async fn evm_block(
		&self,
		block: Arc<SubstrateBlock>,
		hydrated_transactions: bool,
	) -> Option<Block>;

	/// Get the chain ID.
	fn chain_id(&self) -> u64;

	/// Get the max block weight.
	fn max_block_weight(&self) -> Weight;

	/// Get the block notifier, if automine is enabled or `create_block_notifier` was called.
	fn block_notifier(&self) -> Option<tokio::sync::broadcast::Sender<H256>>;

	/// Get the logs matching the given filter.
	async fn logs(&self, filter: Option<Filter>) -> Result<Vec<Log>, ClientError>;

	/// Get the fee history for the given parameters.
	async fn fee_history(
		&self,
		block_count: u32,
		latest_block: BlockNumberOrTag,
		reward_percentiles: Option<Vec<f64>>,
	) -> Result<FeeHistoryResult, ClientError>;

	/// Check if automine is enabled.
	fn is_automine(&self) -> bool;

	/// Get the automine status from the node.
	async fn get_automine(&self) -> bool;
}
