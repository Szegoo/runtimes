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
//! Wires `pallet_psm` into Asset Hub: approved external stablecoins (USDT, Hollar) can be
//! swapped 1:1 for the Asset Hub-issued internal stablecoin up to a governance-controlled
//! issuance cap, with fees routed to a dedicated insurance-fund sub-account. The bootstrap
//! migration that creates the internal stablecoin and seeds initial liquidity lives in the
//! [`migration`] sub-module.

pub mod migration;

use crate::*;

parameter_types! {
	/// `PalletId` deriving the PSM system account that custodies external collateral.
	pub const PsmPalletId: PalletId = PalletId(*b"py/pegsm");
	/// `PalletId` deriving the PSM fee-destination (insurance fund) account.
	pub const PsmFeeDestinationPalletId: PalletId = PalletId(*b"py/psmif");
	/// Account fed by `pallet_psm` minting/redemption fees, derived from
	/// `PsmFeeDestinationPalletId`.
	pub PsmFeeDestination: AccountId =
		PsmFeeDestinationPalletId::get().into_account_truncating();

	/// Maximum number of approved external stablecoins.
	pub const PsmMaxExternalAssets: u32 = 4;

	/// Asset id of the internal stablecoin in the TrustBacked `pallet_assets` instance.
	///
	/// Defaults to `0` and is overwritten by `migration::CreateInternalStable` from
	/// `pallet_assets::NextAssetId` at runtime upgrade.
	pub storage InternalStableAssetId: AssetIdForTrustBackedAssets = 0;
}

/// `TypedGet` returning `PsmManagerLevel::Full`, used with `EnsureRootWithSuccess` to
/// give `Root` full management privilege over `pallet_psm`.
pub struct PsmFullLevel;
impl sp_core::TypedGet for PsmFullLevel {
	type Type = pallet_psm::PsmManagerLevel;
	fn get() -> Self::Type {
		pallet_psm::PsmManagerLevel::Full
	}
}

/// Origin granting `PsmManagerLevel::Emergency` to the `WhitelistedCaller` track.
///
/// To be replaced with a dedicated `MonetaryGuard` track once available.
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

/// Single-asset `fungible` view over `pallet_assets` that `pallet_psm` uses to mint and
/// burn the internal stablecoin.
pub type PsmInternalAsset = fungible::ItemOf<Assets, InternalStableAssetId, AccountId>;

/// Benchmark helper for `pallet_psm`. Fabricates unique sibling-parachain asset `Location`s
/// in `ForeignAssets` (with metadata) so PSM benchmarks exercise the cross-chain mint/redeem
/// path rather than the local TrustBacked instance.
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

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	pub fn assert_keyless_account_id() {
		use sp_core::crypto::{Ss58AddressFormat, Ss58Codec};
		use xcm_executor::traits::ConvertLocation;

		let relay_location = Location::new(1, Junctions::Here);
		let address = LocationToAccountId::convert_location(&relay_location).unwrap();

		let polkadot = Ss58AddressFormat::try_from("polkadot").unwrap();
		let ss58_address = Ss58Codec::to_ss58check_with_version(&address, polkadot);

		// Relay Chain Sovereign Account on Hub.
		assert_eq!(ss58_address, "12pPnA1aFic3ibBh9xMwssM1779vfrJBxqD4mDy8d18r4g95");
	}
}
