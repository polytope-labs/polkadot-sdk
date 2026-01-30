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
//! Trait abstraction for the receipt extractor.
//!
//! This module provides a trait-based abstraction over the receipt extraction functionality,
//! allowing for easier testing, mocking, and potential alternative implementations.
//!
//! # Architecture
//!
//! The `ReceiptExtractorT` trait abstracts the extraction of transaction receipts from
//! Substrate blocks. This enables:
//!
//! - **Testability**: Easy mocking for unit tests
//! - **Flexibility**: Support for different extraction strategies
//! - **Decoupling**: Receipt provider and other components depend on the trait, not concrete types
//!
//! # Implementation
//!
//! The primary implementation is the concrete `ReceiptExtractor` struct which extracts
//! receipts from Substrate extrinsics. Alternative implementations could include:
//! - Mock extractors for testing
//! - Cached extractors
//! - Alternative extraction strategies

use crate::{
	client::{SubstrateBlock, SubstrateBlockNumber},
	ClientError,
};
use jsonrpsee::core::async_trait;
use pallet_revive::evm::{ReceiptInfo, TransactionSigned, H256};

/// A trait abstracting the receipt extraction functionality.
///
/// This trait defines the interface for extracting transaction receipts and related
/// information from Substrate blocks containing Ethereum-compatible transactions.
///
/// # Design Principles
///
/// - **Async-first**: All I/O operations are async for efficient concurrency
/// - **Error handling**: Methods return `Result<T, ClientError>` for fallible operations
/// - **Type safety**: Strongly typed parameters prevent runtime errors
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync + Clone` to support concurrent access and sharing
/// across multiple components.
#[async_trait]
pub trait ReceiptExtractorT: Send + Sync + Clone + 'static {
	/// Check if the block is before the earliest block.
	///
	/// Returns `true` if the given block number is before the earliest block
	/// that should be considered for receipt extraction.
	fn is_before_earliest_block(&self, block_number: SubstrateBlockNumber) -> bool;

	/// Extract receipts from a Substrate block.
	///
	/// Processes all Ethereum transactions in the block and returns their
	/// signed transactions and receipt information.
	///
	/// # Arguments
	///
	/// * `block` - The Substrate block to extract receipts from
	///
	/// # Returns
	///
	/// A vector of tuples containing the signed transaction and its receipt info,
	/// or an error if extraction fails.
	async fn extract_from_block(
		&self,
		block: &SubstrateBlock,
	) -> Result<Vec<(TransactionSigned, ReceiptInfo)>, ClientError>;

	/// Extract a specific transaction and its receipt from a Substrate block.
	///
	/// Retrieves the transaction at the given index within the block and
	/// extracts its signed transaction data and receipt information.
	///
	/// # Arguments
	///
	/// * `block` - The Substrate block containing the transaction
	/// * `transaction_index` - The index of the transaction within the block
	///
	/// # Returns
	///
	/// A tuple containing the signed transaction and its receipt info,
	/// or an error if the transaction is not found or extraction fails.
	async fn extract_from_transaction(
		&self,
		block: &SubstrateBlock,
		transaction_index: usize,
	) -> Result<(TransactionSigned, ReceiptInfo), ClientError>;

	/// Get the Ethereum block hash for the Substrate block.
	///
	/// Retrieves the Ethereum-compatible block hash that corresponds to
	/// the given Substrate block hash and number.
	///
	/// # Arguments
	///
	/// * `block_hash` - The Substrate block hash
	/// * `block_number` - The block number
	///
	/// # Returns
	///
	/// The Ethereum block hash if available, or `None` if not found.
	async fn get_ethereum_block_hash(
		&self,
		block_hash: &H256,
		block_number: u64,
	) -> Option<H256>;
}
