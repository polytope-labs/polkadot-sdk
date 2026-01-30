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
//! Native implementations using Substrate's native traits.
//!
//! This module provides implementations of the RPC traits that use Substrate's
//! native client traits (`HeaderBackend`, `BlockBackend`, `ProvideRuntimeApi`, etc.)
//! instead of subxt. This enables direct embedding of the RPC server into a Substrate node.

pub mod receipt_extractor;

pub use receipt_extractor::NativeReceiptExtractor;
