// Copyright (C) Parity Technologies and the various Polkadot contributors, see Contributions.md
// for a list of specific contributors.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Autogenerated weights for `pallet_election_provider_multi_phase`
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 32.0.0
//! DATE: 2025-01-05, STEPS: `50`, REPEAT: `20`, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! WORST CASE MAP SIZE: `1000000`
//! HOSTNAME: `ggwpez-ref-hw`, CPU: `AMD EPYC 7232P 8-Core Processor`
//! WASM-EXECUTION: `Compiled`, CHAIN: `Some("./kusama-chain-spec.json")`, DB CACHE: 1024

// Executed Command:
// ./target/production/polkadot
// benchmark
// pallet
// --chain=./kusama-chain-spec.json
// --steps=50
// --repeat=20
// --pallet=pallet_election_provider_multi_phase
// --extrinsic=*
// --wasm-execution=compiled
// --heap-pages=4096
// --output=./kusama-weights/
// --header=./file_header.txt

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::Weight};
use core::marker::PhantomData;

/// Weight functions for `pallet_election_provider_multi_phase`.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_election_provider_multi_phase::WeightInfo for WeightInfo<T> {
	/// Storage: `Staking::CurrentEra` (r:1 w:0)
	/// Proof: `Staking::CurrentEra` (`max_values`: Some(1), `max_size`: Some(4), added: 499, mode: `MaxEncodedLen`)
	/// Storage: `Staking::CurrentPlannedSession` (r:1 w:0)
	/// Proof: `Staking::CurrentPlannedSession` (`max_values`: Some(1), `max_size`: Some(4), added: 499, mode: `MaxEncodedLen`)
	/// Storage: `Staking::ErasStartSessionIndex` (r:1 w:0)
	/// Proof: `Staking::ErasStartSessionIndex` (`max_values`: None, `max_size`: Some(16), added: 2491, mode: `MaxEncodedLen`)
	/// Storage: `Babe::EpochIndex` (r:1 w:0)
	/// Proof: `Babe::EpochIndex` (`max_values`: Some(1), `max_size`: Some(8), added: 503, mode: `MaxEncodedLen`)
	/// Storage: `Babe::GenesisSlot` (r:1 w:0)
	/// Proof: `Babe::GenesisSlot` (`max_values`: Some(1), `max_size`: Some(8), added: 503, mode: `MaxEncodedLen`)
	/// Storage: `Babe::CurrentSlot` (r:1 w:0)
	/// Proof: `Babe::CurrentSlot` (`max_values`: Some(1), `max_size`: Some(8), added: 503, mode: `MaxEncodedLen`)
	/// Storage: `Staking::ForceEra` (r:1 w:0)
	/// Proof: `Staking::ForceEra` (`max_values`: Some(1), `max_size`: Some(1), added: 496, mode: `MaxEncodedLen`)
	/// Storage: `ElectionProviderMultiPhase::CurrentPhase` (r:1 w:0)
	/// Proof: `ElectionProviderMultiPhase::CurrentPhase` (`max_values`: Some(1), `max_size`: None, mode: `Measured`)
	fn on_initialize_nothing() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `888`
		//  Estimated: `3481`
		// Minimum execution time: 31_110_000 picoseconds.
		Weight::from_parts(31_950_000, 0)
			.saturating_add(Weight::from_parts(0, 3481))
			.saturating_add(T::DbWeight::get().reads(8))
	}
	/// Storage: `ElectionProviderMultiPhase::Round` (r:1 w:0)
	/// Proof: `ElectionProviderMultiPhase::Round` (`max_values`: Some(1), `max_size`: None, mode: `Measured`)
	/// Storage: `ElectionProviderMultiPhase::CurrentPhase` (r:1 w:1)
	/// Proof: `ElectionProviderMultiPhase::CurrentPhase` (`max_values`: Some(1), `max_size`: None, mode: `Measured`)
	fn on_initialize_open_signed() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `80`
		//  Estimated: `1565`
		// Minimum execution time: 13_020_000 picoseconds.
		Weight::from_parts(13_410_000, 0)
			.saturating_add(Weight::from_parts(0, 1565))
			.saturating_add(T::DbWeight::get().reads(2))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	/// Storage: `ElectionProviderMultiPhase::Round` (r:1 w:0)
	/// Proof: `ElectionProviderMultiPhase::Round` (`max_values`: Some(1), `max_size`: None, mode: `Measured`)
	/// Storage: `ElectionProviderMultiPhase::CurrentPhase` (r:1 w:1)
	/// Proof: `ElectionProviderMultiPhase::CurrentPhase` (`max_values`: Some(1), `max_size`: None, mode: `Measured`)
	fn on_initialize_open_unsigned() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `80`
		//  Estimated: `1565`
		// Minimum execution time: 14_001_000 picoseconds.
		Weight::from_parts(14_290_000, 0)
			.saturating_add(Weight::from_parts(0, 1565))
			.saturating_add(T::DbWeight::get().reads(2))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	/// Storage: `System::Account` (r:1 w:1)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `ElectionProviderMultiPhase::QueuedSolution` (r:0 w:1)
	/// Proof: `ElectionProviderMultiPhase::QueuedSolution` (`max_values`: Some(1), `max_size`: None, mode: `Measured`)
	fn finalize_signed_phase_accept_solution() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `174`
		//  Estimated: `3593`
		// Minimum execution time: 33_970_000 picoseconds.
		Weight::from_parts(34_301_000, 0)
			.saturating_add(Weight::from_parts(0, 3593))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(2))
	}
	/// Storage: `System::Account` (r:1 w:1)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	fn finalize_signed_phase_reject_solution() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `174`
		//  Estimated: `3593`
		// Minimum execution time: 24_571_000 picoseconds.
		Weight::from_parts(24_930_000, 0)
			.saturating_add(Weight::from_parts(0, 3593))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	/// Storage: `ElectionProviderMultiPhase::SnapshotMetadata` (r:0 w:1)
	/// Proof: `ElectionProviderMultiPhase::SnapshotMetadata` (`max_values`: Some(1), `max_size`: None, mode: `Measured`)
	/// Storage: `ElectionProviderMultiPhase::DesiredTargets` (r:0 w:1)
	/// Proof: `ElectionProviderMultiPhase::DesiredTargets` (`max_values`: Some(1), `max_size`: None, mode: `Measured`)
	/// Storage: `ElectionProviderMultiPhase::Snapshot` (r:0 w:1)
	/// Proof: `ElectionProviderMultiPhase::Snapshot` (`max_values`: Some(1), `max_size`: None, mode: `Measured`)
	/// The range of component `v` is `[1000, 2000]`.
	/// The range of component `t` is `[500, 1000]`.
	fn create_snapshot_internal(v: u32, _t: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 742_544_000 picoseconds.
		Weight::from_parts(231_188_401, 0)
			.saturating_add(Weight::from_parts(0, 0))
			// Standard Error: 18_311
			.saturating_add(Weight::from_parts(730_707, 0).saturating_mul(v.into()))
			.saturating_add(T::DbWeight::get().writes(3))
	}
	/// Storage: `ElectionProviderMultiPhase::SignedSubmissionIndices` (r:1 w:1)
	/// Proof: `ElectionProviderMultiPhase::SignedSubmissionIndices` (`max_values`: Some(1), `max_size`: None, mode: `Measured`)
	/// Storage: `ElectionProviderMultiPhase::SignedSubmissionNextIndex` (r:1 w:1)
	/// Proof: `ElectionProviderMultiPhase::SignedSubmissionNextIndex` (`max_values`: Some(1), `max_size`: None, mode: `Measured`)
	/// Storage: `ElectionProviderMultiPhase::SnapshotMetadata` (r:1 w:1)
	/// Proof: `ElectionProviderMultiPhase::SnapshotMetadata` (`max_values`: Some(1), `max_size`: None, mode: `Measured`)
	/// Storage: `ElectionProviderMultiPhase::SignedSubmissionsMap` (r:1 w:0)
	/// Proof: `ElectionProviderMultiPhase::SignedSubmissionsMap` (`max_values`: None, `max_size`: None, mode: `Measured`)
	/// Storage: `ElectionProviderMultiPhase::QueuedSolution` (r:1 w:1)
	/// Proof: `ElectionProviderMultiPhase::QueuedSolution` (`max_values`: Some(1), `max_size`: None, mode: `Measured`)
	/// Storage: `ElectionProviderMultiPhase::Round` (r:1 w:1)
	/// Proof: `ElectionProviderMultiPhase::Round` (`max_values`: Some(1), `max_size`: None, mode: `Measured`)
	/// Storage: `ElectionProviderMultiPhase::CurrentPhase` (r:1 w:1)
	/// Proof: `ElectionProviderMultiPhase::CurrentPhase` (`max_values`: Some(1), `max_size`: None, mode: `Measured`)
	/// Storage: `ElectionProviderMultiPhase::DesiredTargets` (r:0 w:1)
	/// Proof: `ElectionProviderMultiPhase::DesiredTargets` (`max_values`: Some(1), `max_size`: None, mode: `Measured`)
	/// Storage: `ElectionProviderMultiPhase::Snapshot` (r:0 w:1)
	/// Proof: `ElectionProviderMultiPhase::Snapshot` (`max_values`: Some(1), `max_size`: None, mode: `Measured`)
	/// The range of component `a` is `[500, 800]`.
	/// The range of component `d` is `[200, 400]`.
	fn elect_queued(a: u32, d: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `831 + a * (1152 ±0) + d * (47 ±0)`
		//  Estimated: `4281 + a * (1152 ±0) + d * (48 ±0)`
		// Minimum execution time: 737_884_000 picoseconds.
		Weight::from_parts(763_585_000, 0)
			.saturating_add(Weight::from_parts(0, 4281))
			// Standard Error: 9_690
			.saturating_add(Weight::from_parts(489_246, 0).saturating_mul(a.into()))
			.saturating_add(T::DbWeight::get().reads(7))
			.saturating_add(T::DbWeight::get().writes(8))
			.saturating_add(Weight::from_parts(0, 1152).saturating_mul(a.into()))
			.saturating_add(Weight::from_parts(0, 48).saturating_mul(d.into()))
	}
	/// Storage: `ElectionProviderMultiPhase::CurrentPhase` (r:1 w:0)
	/// Proof: `ElectionProviderMultiPhase::CurrentPhase` (`max_values`: Some(1), `max_size`: None, mode: `Measured`)
	/// Storage: `ElectionProviderMultiPhase::Round` (r:1 w:0)
	/// Proof: `ElectionProviderMultiPhase::Round` (`max_values`: Some(1), `max_size`: None, mode: `Measured`)
	/// Storage: `ElectionProviderMultiPhase::SnapshotMetadata` (r:1 w:0)
	/// Proof: `ElectionProviderMultiPhase::SnapshotMetadata` (`max_values`: Some(1), `max_size`: None, mode: `Measured`)
	/// Storage: `ElectionProviderMultiPhase::SignedSubmissionIndices` (r:1 w:1)
	/// Proof: `ElectionProviderMultiPhase::SignedSubmissionIndices` (`max_values`: Some(1), `max_size`: None, mode: `Measured`)
	/// Storage: `ElectionProviderMultiPhase::SignedSubmissionNextIndex` (r:1 w:1)
	/// Proof: `ElectionProviderMultiPhase::SignedSubmissionNextIndex` (`max_values`: Some(1), `max_size`: None, mode: `Measured`)
	/// Storage: `TransactionPayment::NextFeeMultiplier` (r:1 w:0)
	/// Proof: `TransactionPayment::NextFeeMultiplier` (`max_values`: Some(1), `max_size`: Some(16), added: 511, mode: `MaxEncodedLen`)
	/// Storage: `ElectionProviderMultiPhase::SignedSubmissionsMap` (r:0 w:1)
	/// Proof: `ElectionProviderMultiPhase::SignedSubmissionsMap` (`max_values`: None, `max_size`: None, mode: `Measured`)
	fn submit() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1232`
		//  Estimated: `2717`
		// Minimum execution time: 68_791_000 picoseconds.
		Weight::from_parts(69_310_000, 0)
			.saturating_add(Weight::from_parts(0, 2717))
			.saturating_add(T::DbWeight::get().reads(6))
			.saturating_add(T::DbWeight::get().writes(3))
	}
	/// Storage: `ElectionProviderMultiPhase::CurrentPhase` (r:1 w:0)
	/// Proof: `ElectionProviderMultiPhase::CurrentPhase` (`max_values`: Some(1), `max_size`: None, mode: `Measured`)
	/// Storage: `ElectionProviderMultiPhase::Round` (r:1 w:0)
	/// Proof: `ElectionProviderMultiPhase::Round` (`max_values`: Some(1), `max_size`: None, mode: `Measured`)
	/// Storage: `ElectionProviderMultiPhase::DesiredTargets` (r:1 w:0)
	/// Proof: `ElectionProviderMultiPhase::DesiredTargets` (`max_values`: Some(1), `max_size`: None, mode: `Measured`)
	/// Storage: `ElectionProviderMultiPhase::QueuedSolution` (r:1 w:1)
	/// Proof: `ElectionProviderMultiPhase::QueuedSolution` (`max_values`: Some(1), `max_size`: None, mode: `Measured`)
	/// Storage: `ElectionProviderMultiPhase::SnapshotMetadata` (r:1 w:0)
	/// Proof: `ElectionProviderMultiPhase::SnapshotMetadata` (`max_values`: Some(1), `max_size`: None, mode: `Measured`)
	/// Storage: `ElectionProviderMultiPhase::Snapshot` (r:1 w:0)
	/// Proof: `ElectionProviderMultiPhase::Snapshot` (`max_values`: Some(1), `max_size`: None, mode: `Measured`)
	/// Storage: `ElectionProviderMultiPhase::MinimumUntrustedScore` (r:1 w:0)
	/// Proof: `ElectionProviderMultiPhase::MinimumUntrustedScore` (`max_values`: Some(1), `max_size`: None, mode: `Measured`)
	/// The range of component `v` is `[1000, 2000]`.
	/// The range of component `t` is `[500, 1000]`.
	/// The range of component `a` is `[500, 800]`.
	/// The range of component `d` is `[200, 400]`.
	fn submit_unsigned(v: u32, t: u32, a: u32, _d: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `185 + t * (32 ±0) + v * (809 ±0)`
		//  Estimated: `1670 + t * (32 ±0) + v * (809 ±0)`
		// Minimum execution time: 10_071_727_000 picoseconds.
		Weight::from_parts(10_111_958_000, 0)
			.saturating_add(Weight::from_parts(0, 1670))
			// Standard Error: 30_555
			.saturating_add(Weight::from_parts(230_883, 0).saturating_mul(v.into()))
			// Standard Error: 90_548
			.saturating_add(Weight::from_parts(9_151_901, 0).saturating_mul(a.into()))
			.saturating_add(T::DbWeight::get().reads(7))
			.saturating_add(T::DbWeight::get().writes(1))
			.saturating_add(Weight::from_parts(0, 32).saturating_mul(t.into()))
			.saturating_add(Weight::from_parts(0, 809).saturating_mul(v.into()))
	}
	/// Storage: `ElectionProviderMultiPhase::DesiredTargets` (r:1 w:0)
	/// Proof: `ElectionProviderMultiPhase::DesiredTargets` (`max_values`: Some(1), `max_size`: None, mode: `Measured`)
	/// Storage: `ElectionProviderMultiPhase::Snapshot` (r:1 w:0)
	/// Proof: `ElectionProviderMultiPhase::Snapshot` (`max_values`: Some(1), `max_size`: None, mode: `Measured`)
	/// Storage: `ElectionProviderMultiPhase::Round` (r:1 w:0)
	/// Proof: `ElectionProviderMultiPhase::Round` (`max_values`: Some(1), `max_size`: None, mode: `Measured`)
	/// Storage: `ElectionProviderMultiPhase::MinimumUntrustedScore` (r:1 w:0)
	/// Proof: `ElectionProviderMultiPhase::MinimumUntrustedScore` (`max_values`: Some(1), `max_size`: None, mode: `Measured`)
	/// The range of component `v` is `[1000, 2000]`.
	/// The range of component `t` is `[500, 1000]`.
	/// The range of component `a` is `[500, 800]`.
	/// The range of component `d` is `[200, 400]`.
	fn feasibility_check(v: u32, t: u32, a: u32, d: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `160 + t * (32 ±0) + v * (809 ±0)`
		//  Estimated: `1645 + t * (32 ±0) + v * (809 ±0)`
		// Minimum execution time: 8_487_905_000 picoseconds.
		Weight::from_parts(157_331_629, 0)
			.saturating_add(Weight::from_parts(0, 1645))
			// Standard Error: 17_592
			.saturating_add(Weight::from_parts(1_551_064, 0).saturating_mul(v.into()))
			// Standard Error: 58_560
			.saturating_add(Weight::from_parts(10_470_427, 0).saturating_mul(a.into()))
			// Standard Error: 87_770
			.saturating_add(Weight::from_parts(1_217_021, 0).saturating_mul(d.into()))
			.saturating_add(T::DbWeight::get().reads(4))
			.saturating_add(Weight::from_parts(0, 32).saturating_mul(t.into()))
			.saturating_add(Weight::from_parts(0, 809).saturating_mul(v.into()))
	}
}
