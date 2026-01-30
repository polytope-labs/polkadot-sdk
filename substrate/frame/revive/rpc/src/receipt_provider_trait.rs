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
//! Trait abstraction for the receipt provider.
//!
//! This module provides a trait-based abstraction over the receipt provider functionality,
//! allowing for easier testing, mocking, and potential alternative implementations.
//!
//! # Architecture
//!
//! The `ReceiptProviderT` trait abstracts the storage and retrieval of transaction receipts,
//! logs, and block mappings. This enables:
//!
//! - **Testability**: Easy mocking for unit tests
//! - **Flexibility**: Support for different storage backends (SQLite, in-memory, etc.)
//! - **Decoupling**: RPC handlers depend on the trait, not concrete implementations
//!
//! # Implementation
//!
//! The primary implementation is the concrete `ReceiptProvider` struct which uses SQLite
//! for persistent storage. Alternative implementations could include:
//! - Mock providers for testing
//! - In-memory providers for development
//! - Alternative database backends

use crate::{client::{SubstrateBlock, SubstrateBlockHash}, BlockNumberOrTag, ClientError};
use jsonrpsee::core::async_trait;
use pallet_revive::evm::{Filter, Log, ReceiptInfo, TransactionSigned, H256};
use std::collections::HashMap;

/// A trait abstracting the receipt provider functionality.
///
/// This trait defines the interface for storing and retrieving transaction receipts,
/// logs, and block hash mappings between Ethereum and Substrate formats.
///
/// # Design Principles
///
/// - **Async-first**: All I/O operations are async for efficient concurrency
/// - **Error handling**: Methods return `Result<T, E>` for fallible operations
/// - **Type safety**: Strongly typed parameters prevent runtime errors
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync + Clone` to support concurrent access from
/// multiple RPC handlers and sharing across threads.
#[async_trait]
pub trait ReceiptProviderT: Send + Sync + Clone + 'static {
	/// Find a transaction by its hash.
	///
	/// Returns the block hash and transaction index for the given transaction hash.
	///
	/// # Arguments
	///
	/// * `transaction_hash` - The transaction hash to search for
	///
	/// # Returns
	///
	/// A tuple of `(block_hash, transaction_index)` if found, or `None` if not found.
	async fn find_transaction(&self, transaction_hash: &H256) -> Option<(H256, usize)>;

	/// Get the Substrate block hash for the given Ethereum block hash.
	///
	/// Resolves an Ethereum block hash to its corresponding Substrate block hash.
	///
	/// # Arguments
	///
	/// * `ethereum_block_hash` - The Ethereum block hash
	///
	/// # Returns
	///
	/// The corresponding Substrate block hash, or `None` if no mapping exists.
	async fn get_substrate_hash(&self, ethereum_block_hash: &H256) -> Option<H256>;

	/// Get the Ethereum block hash for the given Substrate block hash.
	///
	/// Resolves a Substrate block hash to its corresponding Ethereum block hash.
	///
	/// # Arguments
	///
	/// * `substrate_block_hash` - The Substrate block hash
	///
	/// # Returns
	///
	/// The corresponding Ethereum block hash, or `None` if no mapping exists.
	async fn get_ethereum_hash(&self, substrate_block_hash: &H256) -> Option<H256>;

	/// Check if a block is before the earliest indexed block.
	///
	/// Returns `true` if the given block is before the earliest block that has
	/// receipt data available.
	///
	/// # Arguments
	///
	/// * `at` - The block number or tag to check
	///
	/// # Returns
	///
	/// `true` if the block is before the earliest indexed block, `false` otherwise.
	fn is_before_earliest_block(&self, at: &BlockNumberOrTag) -> bool;

	/// Fetch receipts from a Substrate block.
	///
	/// Extracts all transaction receipts from the given block without storing them.
	///
	/// # Arguments
	///
	/// * `block` - The Substrate block to extract receipts from
	///
	/// # Returns
	///
	/// A vector of tuples containing signed transactions and their receipts,
	/// or an error if extraction fails.
	async fn receipts_from_block(
		&self,
		block: &SubstrateBlock,
	) -> Result<Vec<(TransactionSigned, ReceiptInfo)>, ClientError>;

	/// Extract and insert receipts from a Substrate block.
	///
	/// Extracts receipts from the block and stores them in the provider's storage,
	/// along with the block hash mapping.
	///
	/// # Arguments
	///
	/// * `block` - The Substrate block to process
	/// * `ethereum_hash` - The Ethereum block hash to associate with this block
	///
	/// # Returns
	///
	/// A vector of tuples containing signed transactions and their receipts,
	/// or an error if extraction or insertion fails.
	async fn insert_block_receipts(
		&self,
		block: &SubstrateBlock,
		ethereum_hash: &H256,
	) -> Result<Vec<(TransactionSigned, ReceiptInfo)>, ClientError>;

	/// Get logs matching the given filter.
	///
	/// Queries stored logs based on block range, block hash, addresses, and topics.
	///
	/// # Arguments
	///
	/// * `filter` - Optional filter criteria for logs
	///
	/// # Returns
	///
	/// A vector of matching logs, or an error if the query fails.
	async fn logs(&self, filter: Option<Filter>) -> anyhow::Result<Vec<Log>>;

	/// Get the number of receipts in a block.
	///
	/// Returns the count of transactions in the given block.
	///
	/// # Arguments
	///
	/// * `block_hash` - The block hash to query
	///
	/// # Returns
	///
	/// The number of receipts/transactions in the block, or `None` if the block is not found.
	async fn receipts_count_per_block(&self, block_hash: &SubstrateBlockHash) -> Option<usize>;

	/// Get all transaction hashes in a block.
	///
	/// Returns a mapping of transaction indices to transaction hashes for the given block.
	///
	/// # Arguments
	///
	/// * `block_hash` - The block hash to query
	///
	/// # Returns
	///
	/// A HashMap mapping transaction indices to their hashes, or `None` if the block is not found.
	async fn block_transaction_hashes(&self, block_hash: &H256) -> Option<HashMap<usize, H256>>;

	/// Get a receipt by block hash and transaction index.
	///
	/// Retrieves the receipt for a specific transaction within a block.
	///
	/// # Arguments
	///
	/// * `block_hash` - The block hash containing the transaction
	/// * `transaction_index` - The index of the transaction within the block
	///
	/// # Returns
	///
	/// The receipt info if found, or `None` if not found.
	async fn receipt_by_block_hash_and_index(
		&self,
		block_hash: &H256,
		transaction_index: usize,
	) -> Option<ReceiptInfo>;

	/// Get a receipt by transaction hash.
	///
	/// Finds and retrieves the receipt for a transaction given its hash.
	///
	/// # Arguments
	///
	/// * `transaction_hash` - The transaction hash to look up
	///
	/// # Returns
	///
	/// The receipt info if found, or `None` if not found.
	async fn receipt_by_hash(&self, transaction_hash: &H256) -> Option<ReceiptInfo>;

	/// Get a signed transaction by hash.
	///
	/// Retrieves the signed transaction data for a given transaction hash.
	///
	/// # Arguments
	///
	/// * `transaction_hash` - The transaction hash to look up
	///
	/// # Returns
	///
	/// The signed transaction if found, or `None` if not found.
	async fn signed_tx_by_hash(&self, transaction_hash: &H256) -> Option<TransactionSigned>;
}
