// Copyright 2021 ChainSafe Systems
// SPDX-License-Identifier: LGPL-3.0-only

use crate as pallet;
use crate::{mock::*, SAFTRecord};
use frame_support::{assert_noop, assert_ok};
use primitives::traits::{MultiAssetRegistry, NavProvider};
use sp_runtime::{traits::BadOrigin, FixedPointNumber};
use xcm::v0::{Junction, MultiLocation};

const ASHLEY: AccountId = 0;

#[test]
fn non_admin_cannot_call_any_extrinsics() {
	new_test_ext().execute_with(|| {
		assert_noop!(SaftRegistry::add_saft(Origin::signed(ASHLEY), SAFT_ASSET_ID, 0, 0), BadOrigin);
		assert_noop!(SaftRegistry::remove_saft(Origin::signed(ASHLEY), SAFT_ASSET_ID, 0), BadOrigin);
		assert_noop!(SaftRegistry::report_nav(Origin::signed(ASHLEY), SAFT_ASSET_ID, 0, 0), BadOrigin);
	});
}

#[test]
fn native_asset_disallowed() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			SaftRegistry::add_saft(Origin::signed(ADMIN_ACCOUNT_ID), PINTAssetId::get(), 100, 100),
			pallet_asset_index::Error::<Test>::NativeAssetDisallowed
		);
	});
}

#[test]
fn empty_deposit_does_nothing() {
	new_test_ext().execute_with(|| {
		assert_ok!(SaftRegistry::add_saft(Origin::signed(ADMIN_ACCOUNT_ID), SAFT_ASSET_ID, 0, 0));
		// counter is still at `0`
		assert_eq!(SaftRegistry::saft_counter(SAFT_ASSET_ID), 0);
	});
}

#[test]
fn admin_can_add_and_remove_saft() {
	let units = 20;
	let nav = 100;
	new_test_ext().execute_with(|| {
		// add
		assert_ok!(SaftRegistry::add_saft(Origin::signed(ADMIN_ACCOUNT_ID), SAFT_ASSET_ID, nav, units));
		let counter = SaftRegistry::saft_counter(SAFT_ASSET_ID);
		assert_eq!(counter, 1);
		let saft_id = counter - 1;
		assert_eq!(SaftRegistry::active_safts(SAFT_ASSET_ID, saft_id), Some(SAFTRecord::new(nav, units)));
		// total aggregated NAV
		assert_eq!(SaftRegistry::saft_nav(SAFT_ASSET_ID), nav);

		let additional_nav = 1337;
		let additional_units = 1345;
		assert_ok!(SaftRegistry::add_saft(
			Origin::signed(ADMIN_ACCOUNT_ID),
			SAFT_ASSET_ID,
			additional_nav,
			additional_units
		));
		assert_eq!(
			SaftRegistry::active_safts(SAFT_ASSET_ID, saft_id + 1),
			Some(SAFTRecord::new(additional_nav, additional_units))
		);

		let total_nav = nav + additional_nav;
		let total_units = units + additional_units;
		let expected_mint_token: u128 =
			AssetIndex::nav().unwrap().reciprocal().unwrap().checked_mul_int(total_nav).unwrap();
		let index_token = expected_mint_token + INDEX_TOKEN_SUPPLY;

		assert_eq!(AssetIndex::index_token_equivalent(SAFT_ASSET_ID, total_units).unwrap(), expected_mint_token);
		assert_eq!(AssetIndex::index_total_asset_balance(SAFT_ASSET_ID), total_units);
		assert_eq!(Balances::free_balance(ADMIN_ACCOUNT_ID), index_token);
		assert_eq!(AssetIndex::index_token_balance(&ADMIN_ACCOUNT_ID), index_token);
		assert_eq!(AssetIndex::index_token_issuance(), index_token);

		assert_eq!(SaftRegistry::saft_nav(SAFT_ASSET_ID), total_nav);
		// remove
		assert_ok!(SaftRegistry::remove_saft(Origin::signed(ADMIN_ACCOUNT_ID), SAFT_ASSET_ID, saft_id));
		assert_eq!(SaftRegistry::active_safts(SAFT_ASSET_ID, saft_id), None);
		assert_eq!(SaftRegistry::saft_nav(SAFT_ASSET_ID), additional_nav);
	});
}

#[test]
fn add_saft_depositing_index_tokens_works() {
	let units = 20;
	let nav = 100;
	new_test_ext().execute_with(|| {
		let initial_supply = AssetIndex::index_token_balance(&ADMIN_ACCOUNT_ID);
		assert_ok!(SaftRegistry::add_saft(Origin::signed(ADMIN_ACCOUNT_ID), SAFT_ASSET_ID, nav, units));
		assert_eq!(
			AssetIndex::index_token_balance(&ADMIN_ACCOUNT_ID),
			initial_supply + AssetIndex::index_token_equivalent(SAFT_ASSET_ID, units).unwrap(),
		);
	});
}

#[test]
fn admin_can_add_then_update_saft() {
	new_test_ext().execute_with(|| {
		// add
		let nav = 100;
		let units = 20;
		assert_ok!(SaftRegistry::add_saft(Origin::signed(ADMIN_ACCOUNT_ID), SAFT_ASSET_ID, nav, units));
		assert_eq!(SaftRegistry::active_safts(SAFT_ASSET_ID, 0), Some(SAFTRecord::new(nav, units)));
		assert_eq!(SaftRegistry::saft_nav(SAFT_ASSET_ID), nav);
		// update
		assert_ok!(SaftRegistry::report_nav(Origin::signed(ADMIN_ACCOUNT_ID), SAFT_ASSET_ID, 0, 1000));
		assert_eq!(SaftRegistry::active_safts(SAFT_ASSET_ID, 0), Some(SAFTRecord::new(1000, 20)));
		assert_eq!(SaftRegistry::saft_nav(SAFT_ASSET_ID), 1000);
	});
}

#[test]
fn admin_cannot_update_or_remove_invalid_index() {
	new_test_ext().execute_with(|| {
		// add
		let nav = 1337;
		let units = 13129;
		assert_ok!(SaftRegistry::add_saft(Origin::signed(ADMIN_ACCOUNT_ID), SAFT_ASSET_ID, nav, units));
		let saft_id = 0;
		assert_eq!(SaftRegistry::active_safts(SAFT_ASSET_ID, saft_id), Some(SAFTRecord::new(nav, units)));
		// try update invalid index
		assert_noop!(
			SaftRegistry::report_nav(
				Origin::signed(ADMIN_ACCOUNT_ID),
				SAFT_ASSET_ID,
				1, // invalid saft id
				1000
			),
			pallet::Error::<Test>::SAFTNotFound
		);

		assert_eq!(SaftRegistry::active_safts(SAFT_ASSET_ID, saft_id), Some(SAFTRecord::new(nav, units)));

		// try remove invalid index
		assert_noop!(
			SaftRegistry::remove_saft(
				Origin::signed(ADMIN_ACCOUNT_ID),
				SAFT_ASSET_ID,
				1, // invalid saft id
			),
			pallet::Error::<Test>::SAFTNotFound
		);
		assert_eq!(SaftRegistry::active_safts(SAFT_ASSET_ID, saft_id), Some(SAFTRecord::new(nav, units)));
	});
}

#[test]
fn can_convert_to_liquid() {
	new_test_ext().execute_with(|| {
		// add
		assert_ok!(SaftRegistry::add_saft(Origin::signed(ADMIN_ACCOUNT_ID), SAFT_ASSET_ID, 100, 20));
		assert!(!AssetIndex::is_liquid_asset(&SAFT_ASSET_ID));
		assert_eq!(SaftRegistry::active_safts(SAFT_ASSET_ID, 0), Some(SAFTRecord::new(100, 20)));

		let location: MultiLocation = (Junction::Parent, Junction::Parachain(100)).into();
		assert_ok!(SaftRegistry::convert_to_liquid(Origin::signed(ADMIN_ACCOUNT_ID), SAFT_ASSET_ID, location.clone()));
		assert_eq!(AssetIndex::native_asset_location(&SAFT_ASSET_ID), Some(location));

		// everything is reset and purged
		assert_eq!(SaftRegistry::saft_counter(SAFT_ASSET_ID), 0);
		assert_eq!(SaftRegistry::saft_nav(SAFT_ASSET_ID), 0);
		assert_eq!(SaftRegistry::active_safts(SAFT_ASSET_ID, 0), None);
	});
}
