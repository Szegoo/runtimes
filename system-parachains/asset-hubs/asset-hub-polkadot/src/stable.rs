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

//! Peg Stability Module (PSM) configuration.
//!
//! The internal stablecoin asset id, hard issuance cap, and minimum swap amount are
//! Root-configurable via `dynamic_params::psm` (defined in `lib.rs`).

use crate::*;

parameter_types! {
	/// PalletId for deriving the PSM system account that custodies external collateral.
	pub const PsmPalletId: PalletId = PalletId(*b"py/pegsm");
	/// PalletId for deriving the PSM fee-destination (insurance fund) account.
	pub const PsmFeeDestinationPalletId: PalletId = PalletId(*b"py/psmif");
	pub PsmFeeDestination: AccountId =
		PsmFeeDestinationPalletId::get().into_account_truncating();

	/// Maximum number of approved external stablecoins.
	pub const PsmMaxExternalAssets: u32 = 4;
}

/// `TypedGet` impl returning `PsmManagerLevel::Full`. Used to map `Root` to the full
/// management privilege of `pallet_psm` via `EnsureRootWithSuccess`.
pub struct PsmFullLevel;
impl sp_core::TypedGet for PsmFullLevel {
	type Type = pallet_psm::PsmManagerLevel;
	fn get() -> Self::Type {
		pallet_psm::PsmManagerLevel::Full
	}
}

/// Maps the `WhitelistedCaller` origin to `PsmManagerLevel::Emergency` for the PSM
/// circuit-breaker. Re-using the existing whitelisted-caller track keeps the PR free of
/// new governance plumbing while still distinguishing emergency calls from full
/// management ones at the pallet level.
pub struct PsmEmergencyOrigin;
impl<O> EnsureOrigin<O> for PsmEmergencyOrigin
where
	O: Into<Result<pallet_custom_origins::Origin, O>> + From<pallet_custom_origins::Origin>,
{
	type Success = pallet_psm::PsmManagerLevel;
	fn try_origin(o: O) -> Result<Self::Success, O> {
		o.into().and_then(|o| match o {
			// TODO: use MonetaryGuard origin from new track
			pallet_custom_origins::Origin::WhitelistedCaller =>
				Ok(pallet_psm::PsmManagerLevel::Emergency),
			r => Err(O::from(r)),
		})
	}
	#[cfg(feature = "runtime-benchmarks")]
	fn try_successful_origin() -> Result<O, ()> {
		Ok(O::from(pallet_custom_origins::Origin::WhitelistedCaller))
	}
}

/// Single-asset `fungible` wrapper that the PSM uses to mint/burn the internal stablecoin.
///
/// The TrustBacked asset id is configurable at runtime via the `Parameters` pallet
/// (`dynamic_params::psm::StablecoinAssetId`), so the internal stablecoin's asset id can
/// be set after deployment without a runtime upgrade. The asset itself must still be
/// pre-registered with the PSM-derived account (`PsmPalletId::into_account_truncating()`)
/// as owner/issuer for mint/burn to succeed.
pub type PsmInternalAsset =
	fungible::ItemOf<Assets, dynamic_params::psm::StablecoinAssetId, AccountId>;

/// Benchmark helper for `pallet_psm`. Generates unique foreign-asset `Location`s and
/// creates them in `ForeignAssets` with metadata so the PSM benchmarks can drive
/// mint/redeem flows.
#[cfg(feature = "runtime-benchmarks")]
pub struct PsmBenchmarkHelper;
#[cfg(feature = "runtime-benchmarks")]
impl pallet_psm::BenchmarkHelper<Location, AccountId> for PsmBenchmarkHelper {
	fn get_asset_id(asset_index: u32) -> Location {
		// Each index maps to a unique sibling-parachain Location, ensuring routing
		// through `ForeignAssets` rather than the local TrustBacked instance.
		Location::new(
			1,
			[Junction::Parachain(3_000 + asset_index), Junction::GeneralIndex(asset_index as u128)],
		)
	}
	fn create_asset(asset_id: Location, owner: &AccountId, decimals: u8) {
		use frame_support::traits::fungibles::{
			metadata::Mutate as MetadataMutate, Create, Inspect,
		};
		if !<ForeignAssets as Inspect<AccountId>>::asset_exists(asset_id.clone()) {
			let _ = <ForeignAssets as Create<AccountId>>::create(
				asset_id.clone(),
				owner.clone(),
				true,
				1,
			);
		}
		// Fund the owner so they can pay the metadata deposit.
		let _ = pallet_balances::Pallet::<Runtime>::force_set_balance(
			RuntimeOrigin::root(),
			sp_runtime::MultiAddress::Id(owner.clone()),
			Balance::MAX / 2,
		);
		let _ = <ForeignAssets as MetadataMutate<AccountId>>::set(
			asset_id,
			owner,
			b"Benchmark".to_vec(),
			b"BNC".to_vec(),
			decimals,
		);
	}
}

impl pallet_psm::Config for Runtime {
	type Fungibles = LocalAndForeignAssets;
	type AssetId = Location;
	type MaximumIssuance = dynamic_params::psm::MaximumIssuance;
	type ManagerOrigin =
		EitherOf<EnsureRootWithSuccess<AccountId, PsmFullLevel>, PsmEmergencyOrigin>;
	// TODO: use actual weight info type
	type WeightInfo = ();
	type InternalAsset = PsmInternalAsset;
	type FeeDestination = PsmFeeDestination;
	type PalletId = PsmPalletId;
	type MinSwapAmount = dynamic_params::psm::MinSwapAmount;
	type MaxExternalAssets = PsmMaxExternalAssets;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = PsmBenchmarkHelper;
}
