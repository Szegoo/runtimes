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
//! Split across multiple `OnRuntimeUpgrade` steps so that
//! `pallet_psm::migrations::init::InitializePsm` can interleave between asset creation and
//! the PSM-driven mint. The intended `Unreleased` ordering is:
//!
//! 1. `CreateInternalStable`: `force_create`s the internal stablecoin at the hardcoded
//!    `InternalStableAssetId` with the PSM-derived account as
//!    owner/issuer/admin/freezer, and writes placeholder metadata.
//! 2. `pallet_psm::migrations::init::InitializePsm<Runtime, RuntimePsmInitialConfig>`:
//!    registers USDT and Hollar as approved external assets, snapshots their decimals,
//!    and writes the initial PSM fees.
//! 3. `SeedInternalStableLiquidity`: Treasury PSM-mints `PSM_MINT_AMOUNT` of internal
//!    stable against USDT, creates the DOT/internal-stable pool and seeds it with
//!    `POOL_DOT_AMOUNT` DOT plus `POOL_STABLE_AMOUNT` internal stable, then
//!    reserve-transfers `TO_HYDRATION_AMOUNT` of internal stable to the AH Treasury's
//!    sovereign account on Hydration.
//! 4. `BootstrapHollarBackedStable`: dispatches an XCM to Hydration that withdraws
//!    Treasury's HOLLAR back to AH and schedules a follow-up `pallet_psm::mint` against
//!    HOLLAR `HOLLAR_TRANSFER_DELAY_BLOCKS` blocks later, after HRMP delivery.
//!
//! Failure semantics differ per step. Steps 1, 3, and 4 fail closed and abort the
//! migration so the chain ends in an unambiguous partial state that governance can
//! recover by dispatching the remaining calls directly. Step 2 is upstream-owned and
//! idempotent. In step 3 a `PoolExists` error from `create_pool` is treated as benign so
//! a re-run after an upstream pool already exists is harmless.

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

/// 1.5M HOLLAR @ 18 decimals. Amount the scheduled call PSM-mints once HOLLAR has
/// been pulled back from Hydration to Treasury on AH.
const HOLLAR_PSM_MINT_AMOUNT: Balance = 1_500_000 * 1_000_000_000_000_000_000;

/// 1.5M HOLLAR + 100 HOLLAR (~$100) buffer for XCM execution fees on both legs
/// (Hydration `BuyExecution` and AH-side `BuyExecution`). Surplus stays as dust in
/// Treasury's HOLLAR balance after the mint consumes `HOLLAR_PSM_MINT_AMOUNT`.
const HOLLAR_WITHDRAW_AMOUNT: Balance = 1_500_100 * 1_000_000_000_000_000_000;

/// 100 blocks @ 6s = 10 minutes. Generous margin over the typical HRMP round-trip
/// (4 to 12 blocks) so the scheduled mint doesn't fire before HOLLAR has arrived in
/// Treasury.
const HOLLAR_TRANSFER_DELAY_BLOCKS: BlockNumber = 100;

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

/// Initial PSM parameters consumed by `pallet_psm::migrations::init::InitializePsm`.
///
/// - `max_psm_debt_of_total = 10%`.
/// - For both USDT and Hollar: `(minting_fee = 0%, redemption_fee = 0.01%, ceiling_weight
///   = 100%)`. `minting_fee` MUST stay zero, otherwise `SeedInternalStableLiquidity`
///   (which mints exactly `PSM_MINT_AMOUNT`) would short itself by `fee` for the pool
///   seed and Hydration transfer.
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
/// `force_create`s the asset at the hardcoded `InternalStableAssetId` with the
/// PSM-derived account as owner/issuer/admin/freezer (so `pallet_psm::mint` can issue
/// against it), then writes placeholder metadata.
///
/// `pallet_assets::do_force_create` rejects `id != NextAssetId` when `NextAssetId` is set,
/// so the migration temporarily overrides `NextAssetId` to the chosen id, performs
/// `force_create`, then restores the previous value to keep the auto-increment cursor
/// where it was.
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
/// 3. Reserve-transfers `TO_HYDRATION_AMOUNT` of internal stable to AH Treasury's
///    sovereign account on Hydration.
///
/// Aborts on the first step that fails so the chain ends in an unambiguous partial state;
/// governance can fix the root cause and dispatch the remaining steps directly.
/// `create_pool` returning `pallet_asset_conversion::Error::PoolExists` is treated as
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

/// Bootstrap step 4: pull HOLLAR from Hydration and schedule a PSM mint against it.
///
/// The HOLLAR side of the bootstrap can't fit in a single block: HOLLAR lives on
/// Hydration, so an XCM has to pull it back, HRMP delivery has to land, and only then
/// can `pallet_psm::mint` issue internal stable against it. This migration does the
/// synchronous half (dispatch XCM, schedule the mint); `pallet_scheduler` runs the
/// second half later.
///
/// 1. `pallet_xcm::send` to Hydration: `WithdrawAsset(local_HOLLAR, 1.5M + buffer)`
///    from AH Treasury's sub-sovereign on Hydration, `BuyExecution`, then
///    `DepositReserveAsset` back to AH Treasury.
/// 2. `pallet_scheduler::schedule(at = now + HOLLAR_TRANSFER_DELAY_BLOCKS,
///    call = Utility::dispatch_as(Treasury_signed, pallet_psm::mint(hollar_loc, 1.5M)))`.
///
/// Aborts on either step's error so the chain ends in an unambiguous partial state.
/// If the scheduled mint itself fails when it fires (e.g. HOLLAR didn't arrive in time),
/// governance retries via direct calls.
pub struct BootstrapHollarBackedStable;

impl OnRuntimeUpgrade for BootstrapHollarBackedStable {
	fn on_runtime_upgrade() -> Weight {
		let treasury = pallet_treasury::Pallet::<Runtime>::account_id();

		// XCM payload, written from Hydration's frame:
		//   - `local_HOLLAR` is `(0, [GeneralIndex(222)])` from Hydration's frame.
		//   - `AH` is `(1, [Parachain(1000)])` from Hydration's frame.
		//   - Inner xcm runs on AH; `hollar_location()` is HOLLAR's foreign-asset id on AH.
		let hollar_local_on_hydration =
			Location::new(0, [Junction::GeneralIndex(HOLLAR_ASSET_ID)]);
		let ah_from_hydration = Location::new(1, [Junction::Parachain(ASSET_HUB_PARA_ID)]);
		let beneficiary = Location::new(
			0,
			[Junction::AccountId32 {
				id: <[u8; 32]>::from(treasury.clone()),
				network: None,
			}],
		);

		let withdraw = xcm::v5::Asset {
			id: xcm::v5::AssetId(hollar_local_on_hydration.clone()),
			fun: xcm::v5::Fungibility::Fungible(HOLLAR_WITHDRAW_AMOUNT),
		};
		let inner_xcm: xcm::v5::Xcm<()> = xcm::v5::Xcm(alloc::vec![
			xcm::v5::Instruction::BuyExecution {
				fees: xcm::v5::Asset {
					id: xcm::v5::AssetId(hollar_location()),
					fun: xcm::v5::Fungibility::Fungible(HOLLAR_WITHDRAW_AMOUNT),
				},
				weight_limit: xcm::v5::WeightLimit::Unlimited,
			},
			xcm::v5::Instruction::DepositAsset {
				assets: xcm::v5::AssetFilter::Wild(xcm::v5::WildAsset::AllCounted(1)),
				beneficiary,
			},
		]);
		let xcm: xcm::v5::Xcm<()> = xcm::v5::Xcm(alloc::vec![
			xcm::v5::Instruction::WithdrawAsset(xcm::v5::Assets::from(alloc::vec![
				withdraw.clone()
			])),
			xcm::v5::Instruction::BuyExecution {
				fees: withdraw,
				weight_limit: xcm::v5::WeightLimit::Unlimited,
			},
			xcm::v5::Instruction::DepositReserveAsset {
				assets: xcm::v5::AssetFilter::Wild(xcm::v5::WildAsset::AllCounted(1)),
				dest: ah_from_hydration,
				xcm: inner_xcm,
			},
		]);

		let dest = Location::new(1, [Junction::Parachain(HYDRATION_PARA_ID)]);
		if let Err(e) = pallet_xcm::Pallet::<Runtime>::send(
			RuntimeOrigin::signed(treasury.clone()),
			Box::new(VersionedLocation::from(dest)),
			Box::new(VersionedXcm::from(xcm)),
		) {
			log::error!(
				target: "runtime::stable-init",
				"pallet_xcm::send for HOLLAR pull failed: {:?}; aborting bootstrap migration",
				e,
			);
			return Weight::from_parts(100_000_000, 10_000);
		}
		log::info!(
			target: "runtime::stable-init",
			"Sent XCM to Hydration to withdraw {} HOLLAR base units",
			HOLLAR_WITHDRAW_AMOUNT,
		);

		// Schedule `pallet_psm::mint(hollar_loc, 1.5M)` to fire at `now + 100 blocks`,
		// run as Treasury-signed via `pallet_utility::dispatch_as` (mint requires
		// Signed; scheduler dispatches with whatever origin we encode).
		let signed_treasury_origin: OriginCaller =
			frame_system::RawOrigin::Signed(treasury).into();
		let mint_call: RuntimeCall = RuntimeCall::Psm(pallet_psm::Call::<Runtime>::mint {
			asset_id: hollar_location(),
			external_amount: HOLLAR_PSM_MINT_AMOUNT,
		});
		let dispatch_as_call: RuntimeCall =
			RuntimeCall::Utility(pallet_utility::Call::<Runtime>::dispatch_as {
				as_origin: Box::new(signed_treasury_origin),
				call: Box::new(mint_call),
			});

		let when = frame_system::Pallet::<Runtime>::block_number()
			.saturating_add(HOLLAR_TRANSFER_DELAY_BLOCKS);
		if let Err(e) = pallet_scheduler::Pallet::<Runtime>::schedule(
			RuntimeOrigin::root(),
			when,
			None,
			0,
			Box::new(dispatch_as_call),
		) {
			log::error!(
				target: "runtime::stable-init",
				"pallet_scheduler::schedule for HOLLAR mint failed: {:?}; \
				 aborting bootstrap migration",
				e,
			);
			return Weight::from_parts(100_000_000, 10_000);
		}
		log::info!(
			target: "runtime::stable-init",
			"Scheduled HOLLAR PSM mint of {} for block {}",
			HOLLAR_PSM_MINT_AMOUNT,
			when,
		);

		// TODO: replace with a proper sum of pallet WeightInfo entries.
		Weight::from_parts(1_000_000_000_000, 500_000)
	}

	/// Pre-flight gate: PSM has issuance headroom for the additional 1.5M.
	/// Treasury's HOLLAR sub-sovereign balance on Hydration can't be inspected from
	/// AH state; chopsticks validation covers that.
	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<alloc::vec::Vec<u8>, sp_runtime::TryRuntimeError> {
		use frame_support::ensure;

		// `SeedInternalStableLiquidity` already minted 1.5M internal stable; HOLLAR mint
		// adds another 1.5M (1.5M HOLLAR @ 18 decimals → 1.5M internal stable @ 6 decimals).
		// Total 3M required of headroom against `MaximumIssuance` (default 50M).
		ensure!(
			dynamic_params::psm::MaximumIssuance::get() >= PSM_MINT_AMOUNT.saturating_mul(2),
			"pre_upgrade: MaximumIssuance smaller than combined USDT + HOLLAR mint"
		);

		Ok(alloc::vec::Vec::new())
	}

	/// Asserts an outbound XCM to Hydration was sent and a `pallet_psm::mint` is
	/// queued in `pallet_scheduler` at the expected block.
	#[cfg(feature = "try-runtime")]
	fn post_upgrade(_state: alloc::vec::Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
		use frame_support::ensure;

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
			 HOLLAR pull XCM did not dispatch"
		);

		let when = frame_system::Pallet::<Runtime>::block_number()
			.saturating_add(HOLLAR_TRANSFER_DELAY_BLOCKS);
		let agenda = pallet_scheduler::Agenda::<Runtime>::get(when);
		ensure!(
			!agenda.is_empty(),
			"post_upgrade: pallet_scheduler::Agenda has no entry at the expected block"
		);

		Ok(())
	}
}
