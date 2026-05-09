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

//! One-shot bootstrap of the internal stablecoin and its initial liquidity.
//!
//! Split into two `OnRuntimeUpgrade` steps so that
//! `pallet_psm::migrations::init::InitializePsm` can interleave between asset creation and
//! the PSM-driven mint. The intended `Unreleased` ordering is:
//!
//! 1. [`CreateInternalStable`]: reads `pallet_assets::NextAssetId`, writes `InternalStableAssetId`,
//!    `force_create`s the asset with the PSM-derived account as owner/issuer/admin/freezer, and
//!    writes placeholder metadata.
//! 2. `pallet_psm::migrations::init::InitializePsm<Runtime, `[`RuntimePsmInitialConfig`]`>`:
//!    registers USDT and Hollar as approved external assets, snapshots their decimals, and writes
//!    the initial PSM fees.
//! 3. [`SeedInternalStableLiquidity`]: Treasury PSM-mints `PSM_MINT_AMOUNT` of internal stable
//!    against USDT, creates the DOT/internal-stable pool and seeds it with `POOL_DOT_AMOUNT` DOT
//!    plus `POOL_STABLE_AMOUNT` internal stable, then reserve-transfers `TO_HYDRATION_AMOUNT` of
//!    internal stable to the AH Treasury's sovereign account on Hydration.
//!
//! Failure semantics differ per step. Step 1 fails closed: if the asset cannot be created,
//! nothing downstream is meaningful. Step 2 is upstream-owned and idempotent. Step 3 logs on
//! per-step failure so a missing pool, failed mint, or failed XCM can be retried via
//! governance without re-running the whole sequence.

use super::*;
use crate::*;

use alloc::{boxed::Box, collections::btree_map::BTreeMap};
use frame_support::{pallet_prelude::Weight, traits::OnRuntimeUpgrade};
use sp_runtime::Permill;

const ASSET_HUB_PARA_ID: u32 = 1000;
const HYDRATION_PARA_ID: u32 = 2034;

/// USDT TrustBacked asset id on Asset Hub.
const USDT_ASSET_ID: u128 = 1984;

/// Hollar asset id on Hydration (para 2034).
const HOLLAR_ASSET_ID: u128 = 222;

/// Decimals for the internal stablecoin. Must match `pallet_psm`'s expectation of
/// matching decimals across approved externals (USDT, Hollar, …) and the internal.
const STABLE_DECIMALS: u8 = 6;

/// Existential deposit for the internal stable: 0.01 unit @ 6 decimals.
const STABLE_MIN_BALANCE: Balance = 10_000;

/// On-chain metadata placeholders.
const STABLE_NAME: &[u8] = b"Internal Stable";
const STABLE_SYMBOL: &[u8] = b"TBD";

/// 1.5M USDT @ 6 decimals. Treasury PSM-mints this much internal stable against USDT.
const PSM_MINT_AMOUNT: Balance = 1_500_000 * 1_000_000;

/// 500k DOT @ 10 decimals. Half the seed for the DOT/internal-stable pool.
const POOL_DOT_AMOUNT: Balance = 500_000 * 10_000_000_000;

/// 500k internal stable @ 6 decimals. Other half of the pool seed.
const POOL_STABLE_AMOUNT: Balance = 500_000 * 1_000_000;

/// 1M internal stable @ 6 decimals. Treasury sends this to its sovereign account on
/// Hydration.
const TO_HYDRATION_AMOUNT: Balance = 1_000_000 * 1_000_000;

/// USDT location used by PSM and the seeding migration. Local TrustBacked id 1984.
fn usdt_location() -> Location {
	let assets_pallet_index =
		<Assets as frame_support::pallet_prelude::PalletInfoAccess>::index() as u8;
	Location::new(
		0,
		[Junction::PalletInstance(assets_pallet_index), Junction::GeneralIndex(USDT_ASSET_ID)],
	)
}

/// Hollar location: sibling parachain 2034, asset 222.
fn hollar_location() -> Location {
	Location::new(
		1,
		[Junction::Parachain(HYDRATION_PARA_ID), Junction::GeneralIndex(HOLLAR_ASSET_ID)],
	)
}

/// Initial PSM parameters consumed by `pallet_psm::migrations::init::InitializePsm`:
///
/// - `max_psm_debt_of_total = 10%`.
/// - For both USDT and Hollar: `(minting_fee = 0%, redemption_fee = 0.01%, ceiling_weight = 100%)`.
///   `minting_fee` MUST stay zero, otherwise [`SeedInternalStableLiquidity`] (which mints exactly
///   `PSM_MINT_AMOUNT`) would short itself by `fee` for the pool seed and Hydration transfer.
pub struct RuntimePsmInitialConfig;
impl pallet_psm::migrations::init::InitialPsmConfig<Runtime> for RuntimePsmInitialConfig {
	fn max_psm_debt_of_total() -> Permill {
		Permill::from_percent(10)
	}

	fn asset_configs() -> BTreeMap<Location, (Permill, Permill, Permill)> {
		let cfg: (Permill, Permill, Permill) =
			(Permill::zero(), Permill::from_rational(1u32, 10_000u32), Permill::from_percent(100));
		let mut m = BTreeMap::new();
		m.insert(usdt_location(), cfg);
		m.insert(hollar_location(), cfg);
		m
	}
}

/// Bootstrap step 1: create the internal stable asset.
///
/// `force_create`s the asset at [`InternalStableAssetId`] (`444`) with the PSM-derived
/// account as owner/issuer/admin/freezer (so `pallet_psm::mint` can issue against it),
/// then writes placeholder metadata.
///
/// Aborts on `force_create` failure, since every downstream step requires the asset to
/// exist.
pub struct CreateInternalStable;

impl OnRuntimeUpgrade for CreateInternalStable {
	fn on_runtime_upgrade() -> Weight {
		let stable_id = InternalStableAssetId::get();

		// Owner/issuer/admin/freezer = PSM-derived account so `pallet_psm::mint`
		// can issue against the asset.
		let psm_account: AccountId = PsmPalletId::get().into_account_truncating();

		// `do_force_create` rejects `id != NextAssetId` when it's set (`Error::BadAssetId`).
		// Override, create, then restore so the chain's auto-increment cursor stays where
		// it was (`AutoIncAssetId` would have advanced it on success).
		let saved_next_id = pallet_assets::NextAssetId::<Runtime, TrustBackedAssetsInstance>::get();
		pallet_assets::NextAssetId::<Runtime, TrustBackedAssetsInstance>::put(stable_id);

		let force_create_result =
			pallet_assets::Pallet::<Runtime, TrustBackedAssetsInstance>::force_create(
				RuntimeOrigin::root(),
				codec::Compact(stable_id),
				sp_runtime::MultiAddress::Id(psm_account),
				true, // is_sufficient
				STABLE_MIN_BALANCE,
			);

		match saved_next_id {
			Some(n) => pallet_assets::NextAssetId::<Runtime, TrustBackedAssetsInstance>::put(n),
			None => pallet_assets::NextAssetId::<Runtime, TrustBackedAssetsInstance>::kill(),
		}

		match force_create_result {
			Ok(_) => log::info!(
				target: "runtime::stable-init",
				"Internal stable asset {} force-created (owner = PSM)",
				stable_id,
			),
			Err(e) => {
				log::error!(
					target: "runtime::stable-init",
					"force_create asset {} failed: {:?}; aborting bootstrap migration",
					stable_id,
					e,
				);
				return Weight::from_parts(100_000_000, 10_000);
			},
		}

		match pallet_assets::Pallet::<Runtime, TrustBackedAssetsInstance>::force_set_metadata(
			RuntimeOrigin::root(),
			codec::Compact(stable_id),
			STABLE_NAME.to_vec(),
			STABLE_SYMBOL.to_vec(),
			STABLE_DECIMALS,
			false, // is_frozen
		) {
			Ok(_) => log::info!(
				target: "runtime::stable-init",
				"Internal stable metadata set: name={:?} symbol={:?} decimals={}",
				core::str::from_utf8(STABLE_NAME).unwrap_or("?"),
				core::str::from_utf8(STABLE_SYMBOL).unwrap_or("?"),
				STABLE_DECIMALS,
			),
			Err(e) => log::warn!(
				target: "runtime::stable-init",
				"force_set_metadata failed: {:?}",
				e,
			),
		}

		Weight::from_parts(200_000_000_000, 200_000)
	}

	/// Asserts the hardcoded asset id is free on the live snapshot, otherwise
	/// `force_create` would fail and the bootstrap would abort.
	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<alloc::vec::Vec<u8>, sp_runtime::TryRuntimeError> {
		use frame_support::ensure;

		let stable_id = InternalStableAssetId::get();
		ensure!(
			!pallet_assets::Asset::<Runtime, TrustBackedAssetsInstance>::contains_key(stable_id),
			"pre_upgrade: InternalStableAssetId already exists as an asset; \
			 CreateInternalStable would fail to create it"
		);

		Ok(alloc::vec::Vec::new())
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(_state: alloc::vec::Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
		use frame_support::ensure;

		let stable_id = InternalStableAssetId::get();
		let details = pallet_assets::Asset::<Runtime, TrustBackedAssetsInstance>::get(stable_id)
			.ok_or::<sp_runtime::TryRuntimeError>(
			"post_upgrade: internal stable asset was not created".into(),
		)?;
		let psm_account: AccountId = PsmPalletId::get().into_account_truncating();
		ensure!(
			details.owner == psm_account,
			"post_upgrade: asset owner is not PSM-derived account"
		);
		ensure!(details.is_sufficient, "post_upgrade: asset is not is_sufficient");
		ensure!(
			details.min_balance == STABLE_MIN_BALANCE,
			"post_upgrade: asset min_balance mismatch"
		);

		let metadata =
			pallet_assets::Metadata::<Runtime, TrustBackedAssetsInstance>::get(stable_id);
		ensure!(&metadata.name[..] == STABLE_NAME, "post_upgrade: asset name mismatch");
		ensure!(&metadata.symbol[..] == STABLE_SYMBOL, "post_upgrade: asset symbol mismatch");
		ensure!(metadata.decimals == STABLE_DECIMALS, "post_upgrade: asset decimals mismatch");

		Ok(())
	}
}

/// Bootstrap step 3: produces the initial liquidity once the asset exists and PSM has
/// been initialized.
///
/// 1. Treasury PSM-mints `PSM_MINT_AMOUNT` of internal stable against USDT.
/// 2. Creates the DOT/internal-stable pool and seeds it with `POOL_DOT_AMOUNT` DOT and
///    `POOL_STABLE_AMOUNT` internal stable from Treasury.
/// 3. Reserve-transfers `TO_HYDRATION_AMOUNT` of internal stable to AH Treasury's sovereign account
///    on Hydration.
///
/// Aborts on the first step that fails so the chain ends in an unambiguous partial state;
/// governance can fix the root cause and dispatch the remaining steps directly.
/// `create_pool` returning [`pallet_asset_conversion::Error::PoolExists`] is treated as
/// benign and the migration continues.
pub struct SeedInternalStableLiquidity;

impl OnRuntimeUpgrade for SeedInternalStableLiquidity {
	fn on_runtime_upgrade() -> Weight {
		let stable_id = InternalStableAssetId::get();
		if !pallet_assets::Asset::<Runtime, TrustBackedAssetsInstance>::contains_key(stable_id) {
			log::error!(
				target: "runtime::stable-init",
				"Internal stable asset {} does not exist; \
				 CreateInternalStable did not succeed; skipping seeding step",
				stable_id,
			);
			return Weight::from_parts(100_000_000, 10_000);
		}

		let treasury = pallet_treasury::Pallet::<Runtime>::account_id();
		let origin = RuntimeOrigin::signed(treasury.clone());

		let assets_pallet_index =
			<Assets as frame_support::pallet_prelude::PalletInfoAccess>::index() as u8;
		let stable_loc = Location::new(
			0,
			[
				Junction::PalletInstance(assets_pallet_index),
				Junction::GeneralIndex(stable_id as u128),
			],
		);
		let dot_loc = xcm_config::DotLocation::get();

		if let Err(e) =
			pallet_psm::Pallet::<Runtime>::mint(origin.clone(), usdt_location(), PSM_MINT_AMOUNT)
		{
			log::error!(
				target: "runtime::stable-init",
				"PSM mint failed: {:?}; aborting seeding step",
				e,
			);
			return Weight::from_parts(100_000_000, 10_000);
		}
		log::info!(
			target: "runtime::stable-init",
			"PSM mint OK: {} USDT-base-units swapped for internal stable",
			PSM_MINT_AMOUNT,
		);

		match pallet_asset_conversion::Pallet::<Runtime>::create_pool(
			origin.clone(),
			Box::new(dot_loc.clone()),
			Box::new(stable_loc.clone()),
		) {
			Ok(_) => log::info!(target: "runtime::stable-init", "DOT/internal-stable pool created"),
			Err(e) if e == pallet_asset_conversion::Error::<Runtime>::PoolExists.into() =>
				log::info!(
					target: "runtime::stable-init",
					"DOT/internal-stable pool already exists; continuing",
				),
			Err(e) => {
				log::error!(
					target: "runtime::stable-init",
					"create_pool failed: {:?}; aborting seeding step",
					e,
				);
				return Weight::from_parts(100_000_000, 10_000);
			},
		}

		if let Err(e) = pallet_asset_conversion::Pallet::<Runtime>::add_liquidity(
			origin.clone(),
			Box::new(dot_loc),
			Box::new(stable_loc.clone()),
			POOL_DOT_AMOUNT,
			POOL_STABLE_AMOUNT,
			0,
			0,
			treasury.clone(),
		) {
			log::error!(
				target: "runtime::stable-init",
				"add_liquidity failed: {:?}; aborting seeding step",
				e,
			);
			return Weight::from_parts(100_000_000, 10_000);
		}
		log::info!(
			target: "runtime::stable-init",
			"Pool seeded with 500k DOT / 500k internal stable",
		);

		let dest = Location::new(1, [Junction::Parachain(HYDRATION_PARA_ID)]);
		let beneficiary = Location::new(
			1,
			[
				Junction::Parachain(ASSET_HUB_PARA_ID),
				Junction::AccountId32 { id: <[u8; 32]>::from(treasury), network: None },
			],
		);
		let assets: xcm::v5::Assets = (stable_loc, TO_HYDRATION_AMOUNT).into();

		if let Err(e) = pallet_xcm::Pallet::<Runtime>::limited_reserve_transfer_assets(
			origin,
			Box::new(VersionedLocation::from(dest)),
			Box::new(VersionedLocation::from(beneficiary)),
			Box::new(VersionedAssets::from(assets)),
			0,
			xcm::v5::WeightLimit::Unlimited,
		) {
			log::error!(
				target: "runtime::stable-init",
				"reserve transfer to Hydration failed: {:?}; aborting seeding step",
				e,
			);
			return Weight::from_parts(100_000_000, 10_000);
		}
		log::info!(
			target: "runtime::stable-init",
			"1M internal stable sent to Hydration (AH Treasury sov)",
		);

		// TODO: replace with a proper sum of pallet WeightInfo entries once each step is
		// confirmed to use the expected dispatch path.
		Weight::from_parts(1_800_000_000_000, 800_000)
	}

	/// Pre-flight gates: Treasury holds enough USDT and DOT to cover the mint and pool
	/// seed, and the PSM issuance ceiling accommodates the planned mint.
	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<alloc::vec::Vec<u8>, sp_runtime::TryRuntimeError> {
		use frame_support::ensure;

		let treasury = pallet_treasury::Pallet::<Runtime>::account_id();

		let treasury_usdt = pallet_assets::Pallet::<Runtime, TrustBackedAssetsInstance>::balance(
			USDT_ASSET_ID as AssetIdForTrustBackedAssets,
			&treasury,
		);
		ensure!(
			treasury_usdt >= PSM_MINT_AMOUNT,
			"pre_upgrade: Treasury USDT balance below PSM_MINT_AMOUNT"
		);

		let treasury_dot = pallet_balances::Pallet::<Runtime>::free_balance(&treasury);
		ensure!(
			treasury_dot >= POOL_DOT_AMOUNT,
			"pre_upgrade: Treasury free DOT balance below POOL_DOT_AMOUNT"
		);

		ensure!(
			dynamic_params::psm::MaximumIssuance::get() >= PSM_MINT_AMOUNT,
			"pre_upgrade: dynamic_params::psm::MaximumIssuance smaller than PSM_MINT_AMOUNT"
		);

		Ok(alloc::vec::Vec::new())
	}

	/// Asserts the seeding step's end-state: PSM mint produced the right supply, pool
	/// reserves match, Treasury holds zero internal stable, and the outbound XCM to
	/// Hydration was emitted.
	#[cfg(feature = "try-runtime")]
	fn post_upgrade(_state: alloc::vec::Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
		use frame_support::ensure;

		let stable_id = InternalStableAssetId::get();

		// Total issuance == PSM_MINT_AMOUNT regardless of how the PSM minting fee splits
		// between Treasury and FeeDestination (matching decimals → no rounding).
		let details = pallet_assets::Asset::<Runtime, TrustBackedAssetsInstance>::get(stable_id)
			.ok_or::<sp_runtime::TryRuntimeError>(
			"post_upgrade: internal stable asset missing".into(),
		)?;
		ensure!(
			details.supply == PSM_MINT_AMOUNT,
			"post_upgrade: internal stable total supply != PSM_MINT_AMOUNT \
			 (PSM mint did not produce the expected amount)"
		);

		let assets_pallet_index =
			<Assets as frame_support::pallet_prelude::PalletInfoAccess>::index() as u8;
		let stable_loc = Location::new(
			0,
			[
				Junction::PalletInstance(assets_pallet_index),
				Junction::GeneralIndex(stable_id as u128),
			],
		);
		let dot_loc = xcm_config::DotLocation::get();
		let (dot_reserve, stable_reserve) =
			pallet_asset_conversion::Pallet::<Runtime>::get_reserves(dot_loc, stable_loc).map_err(
				|_| {
					sp_runtime::TryRuntimeError::Other(
						"post_upgrade: DOT/internal-stable pool not found",
					)
				},
			)?;
		ensure!(dot_reserve == POOL_DOT_AMOUNT, "post_upgrade: DOT pool reserve mismatch");
		ensure!(
			stable_reserve == POOL_STABLE_AMOUNT,
			"post_upgrade: internal-stable pool reserve mismatch"
		);

		// mint(1.5M) - pool(500k) - xcm(1M) = 0, exact when PSM minting fee is zero.
		let treasury = pallet_treasury::Pallet::<Runtime>::account_id();
		let treasury_stable = pallet_assets::Pallet::<Runtime, TrustBackedAssetsInstance>::balance(
			stable_id, &treasury,
		);
		ensure!(
			treasury_stable == 0,
			"post_upgrade: Treasury still holds internal stable; \
			 migration accounting drifted (non-zero PSM fee or failed downstream step)"
		);

		let hydration = Location::new(1, [Junction::Parachain(HYDRATION_PARA_ID)]);
		let sent_to_hydration =
			frame_system::Pallet::<Runtime>::read_events_no_consensus().any(|er| match &er.event {
				RuntimeEvent::PolkadotXcm(pallet_xcm::Event::Sent { destination, .. }) =>
					destination == &hydration,
				_ => false,
			});
		ensure!(
			sent_to_hydration,
			"post_upgrade: no pallet_xcm::Sent event for Hydration; \
			 reserve transfer did not execute"
		);

		Ok(())
	}
}
