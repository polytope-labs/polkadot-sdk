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
//! Native receipt extractor implementation using Substrate's native traits.
//!
//! This implementation uses Substrate's `BlockBackend`, `HeaderBackend`, and `ProvideRuntimeApi`
//! traits instead of subxt for better performance and direct node integration.
//!
//! # Implementation Status
//!
//! This is a **stub implementation** that compiles but does not yet fully extract receipts.
//! The following needs to be implemented:
//!
//! ## TODO: Extrinsic Decoding (Critical)
//!
//! We need to decode `Block::Extrinsic` to determine if it's a `Revive::eth_transact` call.
//! Several approaches:
//!
//! 1. **Runtime API Approach** (Recommended): Add a new runtime API method
//!    `receipts_for_block()` that returns all receipts directly from the runtime.
//!    This is the cleanest approach and avoids client-side decoding.
//!
//! 2. **Metadata-based Decoding**: Use the runtime metadata to decode extrinsics.
//!    This requires getting metadata via `Core::metadata()` and using it to identify
//!    and decode `Revive::eth_transact` calls. The `frame-decode` crate might help.
//!
//! 3. **Type-level Approach**: If we know the concrete `Block` and `Call` types at compile
//!    time (which we don't in this generic implementation), we could decode directly.
//!
//! ## TODO: Event Decoding (Critical)
//!
//! Events are stored in `frame_system::Pallet::<Runtime>::events()` storage.
//! We need to:
//!
//! 1. Access the storage key for `System::Events`
//! 2. Decode the events using metadata
//! 3. Filter for `Revive::ContractEmitted` and `Revive::EthExtrinsicRevert` events
//! 4. Match events to their corresponding extrinsic index
//!
//! The storage key is: `twox_128("System") ++ twox_128("Events")`
//!
//! ## TODO: Block Type Conversion
//!
//! The trait expects `SubstrateBlock` (subxt type), but we have native `Block` types.
//! We need a conversion layer or to modify the trait to be generic over block types.

use crate::{
	client::SubstrateBlockNumber, receipt_extractor_trait::ReceiptExtractorT, ClientError, H160,
	LOG_TARGET,
};
use jsonrpsee::core::async_trait;
use pallet_revive::evm::{ReceiptInfo, TransactionSigned, H256};
use sc_client_api::{Backend, BlockBackend, HeaderBackend, StorageProvider};
use sp_api::{ApiExt, CallApiAt, ProvideRuntimeApi};
use sp_runtime::traits::{Block as BlockT, NumberFor};
use std::{marker::PhantomData, sync::Arc};

/// Type alias for the address recovery function.
type RecoverEthAddressFn = Arc<dyn Fn(&TransactionSigned) -> Result<H160, ()> + Send + Sync>;

/// Native receipt extractor using Substrate's native client traits.
///
/// This implementation directly accesses the Substrate node's storage and runtime API
/// without going through subxt, providing better performance and tighter integration.
///
/// # Generic Parameters
///
/// - `Block`: The block type from `sp_runtime::traits::Block`
/// - `Client`: The client type that implements various backend traits
/// - `BE`: The backend type for storage access
///
/// # Current Status
///
/// This is a **partial implementation**. It compiles and provides the correct interface,
/// but the actual receipt extraction logic needs to be implemented. See module-level
/// documentation for TODOs.
#[derive(Clone)]
pub struct NativeReceiptExtractor<Block, Client, BE, AccountId, Balance, Nonce, BlockNumber, Moment>
where
	Block: BlockT,
	Client: HeaderBackend<Block>
		+ BlockBackend<Block>
		+ ProvideRuntimeApi<Block>
		+ StorageProvider<Block, BE>
		+ CallApiAt<Block>
		+ Send
		+ Sync
		+ 'static
		+ Clone,
	Client::Api: pallet_revive::ReviveApi<Block, AccountId, Balance, Nonce, BlockNumber, Moment>
		+ ApiExt<Block>,
	BE: Backend<Block> + 'static + Clone,
	AccountId: codec::Codec + Send + Sync + Clone,
	Balance: codec::Codec + Send + Sync + Clone,
	Nonce: codec::Codec + Send + Sync + Clone,
	BlockNumber: codec::Codec + Send + Sync + Clone,
	Moment: codec::Codec + Send + Sync + Clone,
{
	/// Reference to the native Substrate client.
	client: Arc<Client>,

	/// Reference to the backend for storage access.
	_backend: Arc<BE>,

	/// Earliest block number to consider when searching for transaction receipts.
	earliest_receipt_block: Option<SubstrateBlockNumber>,

	/// Function to recover Ethereum address from transaction signature.
	recover_eth_address: RecoverEthAddressFn,

	/// Phantom data for unused generic type parameters.
	_phantom: PhantomData<(Block, AccountId, Balance, Nonce, BlockNumber, Moment)>,
}

impl<Block, Client, BE, AccountId, Balance, Nonce, BlockNumber, Moment>
	NativeReceiptExtractor<Block, Client, BE, AccountId, Balance, Nonce, BlockNumber, Moment>
where
	Block: BlockT,
	Client: HeaderBackend<Block>
		+ BlockBackend<Block>
		+ ProvideRuntimeApi<Block>
		+ StorageProvider<Block, BE>
		+ CallApiAt<Block>
		+ Send
		+ Sync
		+ 'static
		+ Clone,
	Client::Api: pallet_revive::ReviveApi<Block, AccountId, Balance, Nonce, BlockNumber, Moment>
		+ ApiExt<Block>,
	BE: Backend<Block> + 'static + Clone,
	AccountId: codec::Codec + Send + Sync + Clone,
	Balance: codec::Codec + Send + Sync + Clone,
	Nonce: codec::Codec + Send + Sync + Clone,
	BlockNumber: codec::Codec + Send + Sync + Clone,
	Moment: codec::Codec + Send + Sync + Clone,
{
	/// Create a new native receipt extractor.
	///
	/// # Arguments
	///
	/// * `client` - The Substrate client
	/// * `backend` - The backend for storage access
	/// * `earliest_receipt_block` - Optional earliest block number to index
	pub fn new(
		client: Arc<Client>,
		backend: Arc<BE>,
		earliest_receipt_block: Option<SubstrateBlockNumber>,
	) -> Self {
		Self::new_with_custom_address_recovery(
			client,
			backend,
			earliest_receipt_block,
			Arc::new(|signed_tx: &TransactionSigned| signed_tx.recover_eth_address()),
		)
	}

	/// Create a new native receipt extractor with custom address recovery.
	///
	/// # Arguments
	///
	/// * `client` - The Substrate client
	/// * `backend` - The backend for storage access
	/// * `earliest_receipt_block` - Optional earliest block number to index
	/// * `recover_eth_address_fn` - Custom function to recover Ethereum addresses
	pub fn new_with_custom_address_recovery(
		client: Arc<Client>,
		backend: Arc<BE>,
		earliest_receipt_block: Option<SubstrateBlockNumber>,
		recover_eth_address_fn: RecoverEthAddressFn,
	) -> Self {
		Self {
			client,
			_backend: backend,
			earliest_receipt_block,
			recover_eth_address: recover_eth_address_fn,
			_phantom: PhantomData,
		}
	}

	/// Get the client reference.
	#[allow(dead_code)]
	pub fn client(&self) -> &Arc<Client> {
		&self.client
	}
}

#[async_trait]
impl<Block, Client, BE, AccountId, Balance, Nonce, BlockNumber, Moment> ReceiptExtractorT
	for NativeReceiptExtractor<Block, Client, BE, AccountId, Balance, Nonce, BlockNumber, Moment>
where
	Block: BlockT,
	Client: HeaderBackend<Block>
		+ BlockBackend<Block>
		+ ProvideRuntimeApi<Block>
		+ StorageProvider<Block, BE>
		+ CallApiAt<Block>
		+ Send
		+ Sync
		+ 'static
		+ Clone,
	Client::Api: pallet_revive::ReviveApi<Block, AccountId, Balance, Nonce, BlockNumber, Moment>
		+ ApiExt<Block>,
	BE: Backend<Block> + 'static + Clone,
	AccountId: codec::Codec + Send + Sync + Clone,
	Balance: codec::Codec + Send + Sync + Clone,
	Nonce: codec::Codec + Send + Sync + Clone,
	BlockNumber: codec::Codec + Send + Sync + Clone,
	Moment: codec::Codec + Send + Sync + Clone,
	NumberFor<Block>: Into<u32>,
{
	fn is_before_earliest_block(&self, block_number: SubstrateBlockNumber) -> bool {
		block_number < self.earliest_receipt_block.unwrap_or_default()
	}

	async fn extract_from_block(
		&self,
		_block: &crate::client::SubstrateBlock,
	) -> Result<Vec<(TransactionSigned, ReceiptInfo)>, ClientError> {
		// TODO: Implement receipt extraction from native block
		//
		// This requires:
		// 1. Converting SubstrateBlock (subxt type) to native Block type, OR
		// 2. Modifying the trait to accept a generic block type, OR
		// 3. Creating a bridge type that can work with both
		//
		// Once we have the native block:
		// 1. Get the block hash
		// 2. Call runtime_api.eth_receipt_data(block_hash) to get receipt gas info
		// 3. Decode extrinsics to find Revive::eth_transact calls
		// 4. Match extrinsics with receipt data by index
		// 5. Query storage for events to get logs and revert status
		// 6. Build ReceiptInfo for each transaction
		//
		// For now, return an error indicating this is not yet implemented.

		log::debug!(
			target: LOG_TARGET,
			"Native receipt extraction not yet implemented - requires extrinsic decoding"
		);

		Err(ClientError::NotImplemented(
			"Native extract_from_block requires extrinsic decoding and block type conversion"
				.to_string(),
		))
	}

	async fn extract_from_transaction(
		&self,
		_block: &crate::client::SubstrateBlock,
		_transaction_index: usize,
	) -> Result<(TransactionSigned, ReceiptInfo), ClientError> {
		// TODO: Implement single transaction extraction
		//
		// This is similar to extract_from_block but only processes one transaction.
		// Can be implemented by calling extract_from_block and filtering, or by
		// directly querying for the specific extrinsic.

		log::debug!(
			target: LOG_TARGET,
			"Native transaction extraction not yet implemented"
		);

		Err(ClientError::NotImplemented(
			"Native extract_from_transaction requires extrinsic decoding and block type conversion"
				.to_string(),
		))
	}

	async fn get_ethereum_block_hash(
		&self,
		block_hash: &H256,
		block_number: u64,
	) -> Option<H256> {
		// TODO: Convert H256 to Block::Hash
		//
		// This requires understanding the relationship between:
		// - H256 (Ethereum/subxt hash type)
		// - Block::Hash (Substrate native hash type)
		//
		// Once we have the proper hash type, we can call:
		// self.client.runtime_api().eth_block_hash(at_hash, U256::from(block_number))

		log::debug!(
			target: LOG_TARGET,
			"Native get_ethereum_block_hash not yet implemented - requires hash conversion"
		);

		let _ = (block_hash, block_number);
		None
	}
}

// ================================================================================================
// IMPLEMENTATION NOTES
// ================================================================================================
//
// ## Recommended Implementation Path
//
// ### Phase 1: Runtime API Extension (EASIEST)
//
// Add a new runtime API method to pallet-revive:
//
// ```rust
// sp_api::decl_runtime_apis! {
//     pub trait ReviveApi {
//         fn receipts_for_block() -> Vec<(Vec<u8>, ReceiptInfo)>;
//     }
// }
// ```
//
// This allows the runtime to handle all the complex decoding internally and just
// return the receipts. The client-side code becomes trivial.
//
// ### Phase 2: Client-Side Decoding (MORE FLEXIBLE)
//
// If we need client-side decoding for caching or other reasons:
//
// 1. Get metadata: `client.runtime_api().metadata(at_hash)`
// 2. Decode to find Revive pallet index and eth_transact call index
// 3. Iterate through block.extrinsics()
// 4. Check each extrinsic's pallet/call indices
// 5. Decode the call data to extract the payload
//
// The challenge is that this requires runtime metadata parsing, which subxt does
// well but we want to avoid in native code.
//
// ### Phase 3: Hybrid Approach (PRACTICAL)
//
// Use subxt's metadata decoding utilities even in native code:
//
// ```rust
// use subxt::Metadata;
// let metadata = Metadata::decode(&mut &opaque_metadata.encode()[..])?;
// ```
//
// This gives us the best of both worlds: native client access with proven
// metadata decoding.
//
// ## Storage Access Pattern
//
// To get events from storage:
//
// ```rust
// use sp_core::storage::StorageKey;
// use sp_storage::twox_128;
//
// // System::Events storage key
// let key = StorageKey(
//     [twox_128(b"System"), twox_128(b"Events")].concat()
// );
//
// let events = client.storage(&at_hash, &key)?;
// ```
//
// Then decode the events using metadata.
//
// ================================================================================================
