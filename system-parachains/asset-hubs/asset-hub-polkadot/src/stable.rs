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
//! Wires `pallet_psm` into the Asset Hub runtime: the PSM lets approved external stablecoins
//! (e.g. USDT, Hollar) be swapped 1:1 for the Asset Hub-issued *internal stablecoin* up to
//! a governance-controlled issuance cap, with fees routed to a dedicated insurance-fund
//! sub-account.
//!
//! ## Contents
//!
//! - [`PsmPalletId`] / [`PsmFeeDestinationPalletId`] — sub-account derivation for the PSM custody
//!   account and the fee-destination (insurance fund).
//! - [`InternalStableAssetId`] — storage-backed `AssetId` of the internal stablecoin in the
//!   TrustBacked `pallet_assets` instance. Set by [`migration::InitInternalStableLiquidity`] from
//!   `pallet_assets::NextAssetId` at bootstrap time.
//! - [`PsmFullLevel`] / [`PsmEmergencyOrigin`] — origin-to-`PsmManagerLevel` mapping. `Root` gets
//!   `Full`; the `WhitelistedCaller` track gets `Emergency` (to be replaced with a dedicated
//!   `MonetaryGuard` track).
//! - [`PsmInternalAsset`] — single-asset `fungible` view over `pallet_assets` that `pallet_psm`
//!   uses to mint/burn the internal stablecoin.
//! - [`PsmBenchmarkHelper`] — runtime-benchmarks helper that fabricates unique foreign-asset
//!   `Location`s in `ForeignAssets` so PSM benchmarks exercise the cross-chain path.
//! - [`migration`] — one-shot bootstrap that creates the internal stablecoin, PSM-mints against
//!   USDT, seeds the DOT/internal-stable pool, and pushes initial liquidity to Hydration. See the
//!   module docs for details.

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

	/// Asset id of the internal stablecoin.
	pub storage InternalStableAssetId: AssetIdForTrustBackedAssets = 0;
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
pub type PsmInternalAsset = fungible::ItemOf<Assets, InternalStableAssetId, AccountId>;

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

/// One-shot bootstrap of the internal stablecoin and its initial liquidity.
///
/// Runs as a single-block `OnRuntimeUpgrade` and performs, in order:
///
/// 1. Reads the next auto-incremented id from `pallet_assets::NextAssetId` and stores it in
///    [`InternalStableAssetId`]. Aborts (with logged error) if unset.
/// 2. `force_create`s the asset with the PSM-derived account as owner/issuer/admin/freezer so that
///    [`pallet_psm`] can mint against it, and `force_set_metadata` writes the placeholder
///    name/symbol/decimals.
/// 3. Treasury PSM-mints [`PSM_MINT_AMOUNT`] of internal stable against USDT (TrustBacked id
///    [`USDT_ASSET_ID`]).
/// 4. Creates the DOT/internal-stable pool in `pallet_asset_conversion` and seeds it with
///    [`POOL_DOT_AMOUNT`] DOT + [`POOL_STABLE_AMOUNT`] internal stable from Treasury.
/// 5. Reserve-transfers [`TO_HYDRATION_AMOUNT`] of internal stable to the AH Treasury's sovereign
///    account on Hydration (para `2034`), resolved via Hydration's `HashedDescription`
///    `LocationToAccountId`.
///
/// Steps 2–5 log on failure rather than aborting: the asset must exist for anything else to
/// be meaningful, but a missing pool, failed mint, or failed XCM transfer can be retried via
/// governance without re-running the whole migration.
pub mod migration {
	use super::*;
	use alloc::boxed::Box;
	use frame_support::{pallet_prelude::Weight, traits::OnRuntimeUpgrade};

	const ASSET_HUB_PARA_ID: u32 = 1000;
	const HYDRATION_PARA_ID: u32 = 2034;

	/// USDT TrustBacked asset id on Asset Hub.
	const USDT_ASSET_ID: u128 = 1984;

	/// Decimals for the internal stablecoin. Must match `pallet_psm`'s expectation of
	/// matching decimals across approved externals (USDT, Hollar, …) and the internal.
	const STABLE_DECIMALS: u8 = 6;

	/// Existential deposit for the internal stable: 0.01 unit @ 6 decimals.
	const STABLE_MIN_BALANCE: Balance = 10_000;

	/// On-chain metadata placeholders.
	const STABLE_NAME: &[u8] = b"Internal Stable";
	const STABLE_SYMBOL: &[u8] = b"TBD";

	/// 1.5M USDT @ 6 decimals — Treasury PSM-mints this much internal stable against USDT.
	const PSM_MINT_AMOUNT: Balance = 1_500_000 * 1_000_000;

	/// 500k DOT @ 10 decimals — half the seed for the DOT/internal-stable pool.
	const POOL_DOT_AMOUNT: Balance = 500_000 * 10_000_000_000;

	/// 500k internal stable @ 6 decimals — other half of the pool seed.
	const POOL_STABLE_AMOUNT: Balance = 500_000 * 1_000_000;

	/// 1M internal stable @ 6 decimals — Treasury sends this to its sov on Hydration.
	const TO_HYDRATION_AMOUNT: Balance = 1_000_000 * 1_000_000;

	pub struct InitInternalStableLiquidity;

	impl OnRuntimeUpgrade for InitInternalStableLiquidity {
		fn on_runtime_upgrade() -> Weight {
			let treasury = pallet_treasury::Pallet::<Runtime>::account_id();
			let origin = RuntimeOrigin::signed(treasury.clone());

			let assets_pallet_index =
				<Assets as frame_support::pallet_prelude::PalletInfoAccess>::index() as u8;
			let usdt_loc = Location::new(
				0,
				[
					Junction::PalletInstance(assets_pallet_index),
					Junction::GeneralIndex(USDT_ASSET_ID),
				],
			);
			let dot_loc = xcm_config::DotLocation::get();

			// Take the next auto-incremented asset id from `pallet_assets`. If it's unset,
			// abandon the migration.
			let stable_id: AssetIdForTrustBackedAssets =
				match pallet_assets::NextAssetId::<Runtime, TrustBackedAssetsInstance>::get() {
					Some(id) => id,
					None => {
						log::error!(
							target: "runtime::stable-init",
							"NextAssetId unset; aborting bootstrap migration",
						);
						return Weight::from_parts(100_000_000, 10_000);
					},
				};

			InternalStableAssetId::set(&stable_id);

			// Owner/issuer/admin/freezer = PSM-derived account so `pallet_psm::mint`
			// can issue against the asset.
			let psm_account: AccountId = PsmPalletId::get().into_account_truncating();

			// Hard precondition: if the asset can't be created, every downstream step
			// (mint, pool, cross-chain transfer) is meaningless — abort the migration.
			match pallet_assets::Pallet::<Runtime, TrustBackedAssetsInstance>::force_create(
				RuntimeOrigin::root(),
				codec::Compact(stable_id),
				sp_runtime::MultiAddress::Id(psm_account.clone()),
				true, // is_sufficient
				STABLE_MIN_BALANCE,
			) {
				Ok(_) => log::info!(
					target: "runtime::stable-init",
					"Internal stable asset {} force-created (owner = PSM)",
					stable_id,
				),
				Err(e) => {
					log::error!(
						target: "runtime::stable-init",
						"force_create asset {} failed: {:?} — aborting bootstrap migration",
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

			let stable_loc = Location::new(
				0,
				[
					Junction::PalletInstance(assets_pallet_index),
					Junction::GeneralIndex(stable_id as u128),
				],
			);

			// Treasury PSM-mints internal stable against 1.5M USDT.
			match pallet_psm::Pallet::<Runtime>::mint(origin.clone(), usdt_loc, PSM_MINT_AMOUNT) {
				Ok(_) => log::info!(
					target: "runtime::stable-init",
					"PSM mint OK: {} USDT-base-units swapped for internal stable",
					PSM_MINT_AMOUNT,
				),
				Err(e) => log::warn!(
					target: "runtime::stable-init",
					"PSM mint failed: {:?}",
					e,
				),
			}

			// Create the DOT/internal-stable pool (idempotent — `PoolExists` is benign)
			// and add 500k+500k liquidity owned by Treasury.
			match pallet_asset_conversion::Pallet::<Runtime>::create_pool(
				origin.clone(),
				Box::new(dot_loc.clone()),
				Box::new(stable_loc.clone()),
			) {
				Ok(_) => log::info!(
					target: "runtime::stable-init",
					"DOT/internal-stable pool created",
				),
				Err(e) => log::warn!(
					target: "runtime::stable-init",
					"create_pool failed (may already exist): {:?}",
					e,
				),
			}

			match pallet_asset_conversion::Pallet::<Runtime>::add_liquidity(
				origin.clone(),
				Box::new(dot_loc),
				Box::new(stable_loc.clone()),
				POOL_DOT_AMOUNT,
				POOL_STABLE_AMOUNT,
				0,
				0,
				treasury.clone(),
			) {
				Ok(_) => log::info!(
					target: "runtime::stable-init",
					"Pool seeded with 500k DOT / 500k internal stable",
				),
				Err(e) => log::warn!(
					target: "runtime::stable-init",
					"add_liquidity failed: {:?}",
					e,
				),
			}

			// Reserve-transfer 1M internal stable to AH Treasury's sov on Hydration.
			// `dest`        — Hydration, from AH's frame.
			// `beneficiary` — AH Treasury location, written from Hydration's frame
			//                 (`(1, [Parachain(1000), AccountId32(<treasury>)])`).
			//                 Hydration's `LocationToAccountId` (HashedDescription)
			//                 resolves it to a deterministic sovereign address controlled
			//                 by AH Treasury via `Transact`.
			let dest = Location::new(1, [Junction::Parachain(HYDRATION_PARA_ID)]);
			let beneficiary = Location::new(
				1,
				[
					Junction::Parachain(ASSET_HUB_PARA_ID),
					Junction::AccountId32 { id: <[u8; 32]>::from(treasury.clone()), network: None },
				],
			);
			let assets: xcm::v5::Assets = (stable_loc, TO_HYDRATION_AMOUNT).into();

			match pallet_xcm::Pallet::<Runtime>::limited_reserve_transfer_assets(
				origin,
				Box::new(VersionedLocation::from(dest)),
				Box::new(VersionedLocation::from(beneficiary)),
				Box::new(VersionedAssets::from(assets)),
				0,
				xcm::v5::WeightLimit::Unlimited,
			) {
				Ok(_) => log::info!(
					target: "runtime::stable-init",
					"1M internal stable sent to Hydration (AH Treasury sov)",
				),
				Err(e) => log::warn!(
					target: "runtime::stable-init",
					"reserve transfer to Hydration failed: {:?}",
					e,
				),
			}

			// TODO: replace with a proper sum of pallet WeightInfo entries once each step
			// is confirmed to use the expected dispatch path. The block where this
			// migration runs will already include normal block work, so leave headroom.
			Weight::from_parts(2_000_000_000_000, 1_000_000)
		}

		/// Capture preconditions and pre-state for [`Self::post_upgrade`].
		///
		/// Hard-fails if any precondition needed by the bootstrap is missing on the snapshot —
		/// the migration's `on_runtime_upgrade` swallows most failures with `log::warn`, so
		/// these `ensure!`s are the only signal that the live state is incompatible.
		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<alloc::vec::Vec<u8>, sp_runtime::TryRuntimeError> {
			use codec::Encode;
			use frame_support::ensure;

			let treasury = pallet_treasury::Pallet::<Runtime>::account_id();

			let next_id = pallet_assets::NextAssetId::<Runtime, TrustBackedAssetsInstance>::get()
				.ok_or::<sp_runtime::TryRuntimeError>(
					"pre_upgrade: pallet_assets::NextAssetId is unset; \
					 InitInternalStableLiquidity would silently abort"
						.into(),
				)?;

			ensure!(
				!pallet_assets::Asset::<Runtime, TrustBackedAssetsInstance>::contains_key(next_id),
				"pre_upgrade: NextAssetId points at an already-existing asset"
			);

			ensure!(
				InternalStableAssetId::get() == 0,
				"pre_upgrade: InternalStableAssetId already set; migration appears to have run before"
			);

			let treasury_usdt =
				pallet_assets::Pallet::<Runtime, TrustBackedAssetsInstance>::balance(
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

			Ok((next_id, treasury_usdt, treasury_dot).encode())
		}

		/// Verify the bootstrap landed every step it advertises. Decodes pre-state from
		/// `pre_upgrade` and asserts asset creation, PSM mint, AMM seed, and the outbound
		/// XCM to Hydration. Hard-fails on any deviation.
		#[cfg(feature = "try-runtime")]
		fn post_upgrade(
			state: alloc::vec::Vec<u8>,
		) -> Result<(), sp_runtime::TryRuntimeError> {
			use codec::Decode;
			use frame_support::ensure;

			let (pre_next_id, pre_usdt, pre_dot): (
				AssetIdForTrustBackedAssets,
				Balance,
				Balance,
			) = Decode::decode(&mut &state[..]).map_err(|_| {
				sp_runtime::TryRuntimeError::Other("post_upgrade: pre-state decode failed")
			})?;

			let stable_id = InternalStableAssetId::get();
			ensure!(
				stable_id == pre_next_id,
				"post_upgrade: InternalStableAssetId does not match pre-upgrade NextAssetId"
			);
			ensure!(stable_id != 0, "post_upgrade: InternalStableAssetId still default");

			let details =
				pallet_assets::Asset::<Runtime, TrustBackedAssetsInstance>::get(stable_id)
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
			ensure!(
				&metadata.name[..] == STABLE_NAME,
				"post_upgrade: asset name mismatch"
			);
			ensure!(
				&metadata.symbol[..] == STABLE_SYMBOL,
				"post_upgrade: asset symbol mismatch"
			);
			ensure!(
				metadata.decimals == STABLE_DECIMALS,
				"post_upgrade: asset decimals mismatch"
			);

			// Total issuance == PSM_MINT_AMOUNT regardless of how the PSM minting fee splits
			// between Treasury and FeeDestination (matching decimals → no rounding). If this
			// is 0 the PSM mint never executed — most likely USDT was not registered as a
			// PSM external asset.
			ensure!(
				details.supply == PSM_MINT_AMOUNT,
				"post_upgrade: internal stable total supply != PSM_MINT_AMOUNT \
				 (PSM mint did not produce the expected amount; check USDT registration)"
			);

			let treasury = pallet_treasury::Pallet::<Runtime>::account_id();
			let treasury_usdt_post =
				pallet_assets::Pallet::<Runtime, TrustBackedAssetsInstance>::balance(
					USDT_ASSET_ID as AssetIdForTrustBackedAssets,
					&treasury,
				);
			ensure!(
				pre_usdt.saturating_sub(treasury_usdt_post) == PSM_MINT_AMOUNT,
				"post_upgrade: Treasury USDT decrement != PSM_MINT_AMOUNT"
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
				pallet_asset_conversion::Pallet::<Runtime>::get_reserves(
					dot_loc,
					stable_loc,
				)
				.map_err(|_| {
					sp_runtime::TryRuntimeError::Other(
						"post_upgrade: DOT/internal-stable pool not found",
					)
				})?;
			ensure!(
				dot_reserve == POOL_DOT_AMOUNT,
				"post_upgrade: DOT pool reserve mismatch"
			);
			ensure!(
				stable_reserve == POOL_STABLE_AMOUNT,
				"post_upgrade: internal-stable pool reserve mismatch"
			);

			let treasury_dot_post = pallet_balances::Pallet::<Runtime>::free_balance(&treasury);
			ensure!(
				pre_dot.saturating_sub(treasury_dot_post) >= POOL_DOT_AMOUNT,
				"post_upgrade: Treasury DOT decrement smaller than POOL_DOT_AMOUNT"
			);

			// mint(1.5M) - pool(500k) - xcm(1M) = 0, exact when PSM minting fee is zero.
			let treasury_stable =
				pallet_assets::Pallet::<Runtime, TrustBackedAssetsInstance>::balance(
					stable_id, &treasury,
				);
			ensure!(
				treasury_stable == 0,
				"post_upgrade: Treasury still holds internal stable; \
				 migration accounting drifted (non-zero PSM fee or failed downstream step)"
			);

			let hydration = Location::new(1, [Junction::Parachain(HYDRATION_PARA_ID)]);
			let sent_to_hydration = frame_system::Pallet::<Runtime>::read_events_no_consensus()
				.any(|er| match &er.event {
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
