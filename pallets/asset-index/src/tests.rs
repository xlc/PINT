// Copyright 2021 ChainSafe Systems
// SPDX-License-Identifier: LGPL-3.0-only

use frame_support::{assert_noop, assert_ok, traits::Currency as _};
use orml_traits::MultiCurrency;
use rand::Rng;
use sp_runtime::{traits::Zero, FixedPointNumber};
use xcm::v0::MultiLocation;

use pallet_price_feed::PriceFeed;
use primitives::{
	traits::{AssetRecorder, NavProvider},
	AssetAvailability, Price,
};

use crate as pallet;
use crate::{mock::*, types::DepositRange};

#[test]
fn can_add_asset() {
	new_test_ext().execute_with(|| {
		assert_ok!(AssetIndex::add_asset(Origin::signed(ACCOUNT_ID), ASSET_A_ID, 100, MultiLocation::Null, 5));
		assert_eq!(pallet::Assets::<Test>::get(ASSET_A_ID), Some(AssetAvailability::Liquid(MultiLocation::Null)));
		assert_eq!(AssetIndex::index_total_asset_balance(ASSET_A_ID), 100);

		assert_eq!(Balances::free_balance(ACCOUNT_ID), 5);

		assert_eq!(AssetIndex::index_token_balance(&ACCOUNT_ID), 5);
		assert_eq!(AssetIndex::index_token_issuance(), 5);
	});
}

#[test]
fn native_asset_disallowed() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			AssetIndex::add_asset(Origin::signed(ACCOUNT_ID), PINT_ASSET_ID, 100, MultiLocation::Null, 5),
			pallet::Error::<Test>::NativeAssetDisallowed
		);
	});
}

#[test]
fn can_add_asset_twice_and_units_accumulate() {
	new_test_ext().execute_with(|| {
		assert_ok!(AssetIndex::add_asset(Origin::signed(ACCOUNT_ID), ASSET_A_ID, 100, MultiLocation::Null, 5));
		assert_ok!(AssetIndex::add_asset(Origin::signed(ACCOUNT_ID), ASSET_A_ID, 100, MultiLocation::Null, 5));
		assert_eq!(pallet::Assets::<Test>::get(ASSET_A_ID), Some(AssetAvailability::Liquid(MultiLocation::Null)));
		assert_eq!(AssetIndex::index_total_asset_balance(ASSET_A_ID), 200);
		assert_eq!(Balances::free_balance(ACCOUNT_ID), 10);
	});
}

#[test]
fn can_set_metadata() {
	new_test_ext().execute_with(|| {
		assert_ok!(AssetIndex::set_metadata(
			Origin::signed(ACCOUNT_ID),
			ASSET_A_ID,
			b"dot".to_vec(),
			b"dot".to_vec(),
			8,
		));

		assert_noop!(
			AssetIndex::set_metadata(Origin::signed(ACCOUNT_ID), ASSET_A_ID, b"".to_vec(), b"dot".to_vec(), 8,),
			pallet::Error::<Test>::BadMetadata
		);

		assert_noop!(
			AssetIndex::set_metadata(Origin::signed(ACCOUNT_ID), ASSET_A_ID, b"dot".to_vec(), b"".to_vec(), 8,),
			pallet::Error::<Test>::BadMetadata
		);

		assert_noop!(
			AssetIndex::set_metadata(Origin::signed(ACCOUNT_ID), ASSET_A_ID, b"dot".to_vec(), b"dot".to_vec(), 13),
			pallet::Error::<Test>::InvalidDecimals
		);
	});
}

#[test]
fn can_update_metadata() {
	new_test_ext().execute_with(|| {
		assert_ok!(AssetIndex::set_metadata(
			Origin::signed(ACCOUNT_ID),
			ASSET_A_ID,
			b"dot".to_vec(),
			b"dot".to_vec(),
			8,
		));

		assert_eq!(<pallet::Metadata<Test>>::get(ASSET_A_ID).name, b"dot".to_vec());

		assert_ok!(AssetIndex::set_metadata(
			Origin::signed(ACCOUNT_ID),
			ASSET_A_ID,
			b"pint".to_vec(),
			b"pint".to_vec(),
			8,
		));

		assert_eq!(<pallet::Metadata<Test>>::get(ASSET_A_ID).name, b"pint".to_vec());
	});
}

#[test]
fn can_add_saft() {
	let units = 5;
	let saft_nav = 100_000;
	new_test_ext().execute_with(|| {
		// we first need to contribute some assets in order to mint an intial supply of pint
		let initial_liquid_supply = 1_000;
		let initial_tokens = 2_0000;
		assert_ok!(AssetIndex::add_asset(
			Origin::signed(ACCOUNT_ID),
			ASSET_A_ID,
			initial_liquid_supply,
			MultiLocation::Null,
			initial_tokens
		));
		let index_token_supply = AssetIndex::index_token_issuance();
		assert_eq!(index_token_supply, initial_tokens);
		let price = MockPriceFeed::get_price(ASSET_A_ID).unwrap();
		let value = price.checked_mul_int(initial_liquid_supply).unwrap();
		assert_eq!(AssetIndex::net_asset_value(ASSET_A_ID).unwrap(), value);
		let nav = AssetIndex::nav().unwrap();
		assert_eq!(nav, Price::checked_from_rational(value, index_token_supply).unwrap());

		// now we can add a SAFT
		let saft_lp = 99;
		assert_ok!(AssetIndex::add_saft(&saft_lp, SAFT_ASSET_ID, units, saft_nav));
		assert_eq!(pallet::Assets::<Test>::get(SAFT_ASSET_ID), Some(AssetAvailability::Saft));
		assert_eq!(AssetIndex::index_total_asset_balance(SAFT_ASSET_ID), units);

		let expected_minted_token = nav.reciprocal().unwrap().saturating_mul_int(saft_nav);

		assert_eq!(AssetIndex::index_token_balance(&saft_lp), expected_minted_token);
		assert_eq!(AssetIndex::index_token_issuance(), index_token_supply + expected_minted_token);
	});
}

#[test]
fn add_saft_fails_on_liquid_already_registered() {
	let balance = vec![(ACCOUNT_ID, UNKNOWN_ASSET_ID, 1000)];
	new_test_ext_with_balance(balance).execute_with(|| {
		assert_ok!(AssetIndex::add_asset(Origin::signed(ACCOUNT_ID), UNKNOWN_ASSET_ID, 100, MultiLocation::Null, 5));
		assert_noop!(AssetIndex::add_saft(&ACCOUNT_ID, UNKNOWN_ASSET_ID, 100, 5), pallet::Error::<Test>::ExpectedSAFT);
	})
}

#[test]
fn deposit_only_works_for_added_liquid_assets() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			AssetIndex::deposit(Origin::signed(ASHLEY), ASSET_A_ID, 1_000),
			pallet::Error::<Test>::UnsupportedAsset
		);

		// mint and intial supply of pint
		let initial_liquid_supply = 1_000;
		let initial_tokens = 2_0000;
		assert_ok!(AssetIndex::add_asset(
			Origin::signed(ACCOUNT_ID),
			ASSET_A_ID,
			initial_liquid_supply,
			MultiLocation::Null,
			initial_tokens
		));

		// add saft
		assert_ok!(AssetIndex::add_saft(&ACCOUNT_ID, ASSET_B_ID, 100, 5));
		assert_noop!(
			AssetIndex::deposit(Origin::signed(ASHLEY), ASSET_B_ID, 1_000),
			pallet::Error::<Test>::UnsupportedAsset
		);
	});
}

#[test]
fn deposit_fail_for_native_asset() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			AssetIndex::deposit(Origin::signed(ASHLEY), PINT_ASSET_ID, 1_000),
			pallet::Error::<Test>::NativeAssetDisallowed
		);
	});
}

#[test]
fn deposit_fails_for_unknown_assets() {
	new_test_ext().execute_with(|| {
		assert_ok!(AssetIndex::add_asset(Origin::signed(ACCOUNT_ID), ASSET_A_ID, 100, MultiLocation::Null, 5));
		assert_noop!(
			AssetIndex::deposit(Origin::signed(ASHLEY), UNKNOWN_ASSET_ID, 1_000),
			pallet::Error::<Test>::UnsupportedAsset
		);
	})
}

#[test]
fn can_calculate_nav_upon_deposit() {
	new_test_ext().execute_with(|| {
		MockPriceFeed::set_prices(vec![(ASSET_A_ID, Price::saturating_from_rational(1, 2))]);
		// no index tokens, NAV == 0
		assert!(AssetIndex::nav().unwrap().is_zero());
		assert!(AssetIndex::liquid_nav().unwrap().is_zero());
		assert!(AssetIndex::saft_nav().unwrap().is_zero());

		let index_token_units = 5;
		let asset_amount = 100;
		let asset_price = MockPriceFeed::get_price(ASSET_A_ID).unwrap();
		assert_ok!(AssetIndex::add_asset(
			Origin::signed(ACCOUNT_ID),
			ASSET_A_ID,
			asset_amount,
			MultiLocation::Null,
			index_token_units
		));

		// per token value (NAV) is now the value of all deposited assets over index token supply
		let nav = AssetIndex::nav().unwrap();
		let asset_value = asset_price.saturating_mul_int(asset_amount);
		let expected = Price::saturating_from_rational(asset_value, index_token_units);
		assert_eq!(nav, expected);
		// liquid only
		let liquid_nav = AssetIndex::liquid_nav().unwrap();
		assert_eq!(nav, liquid_nav);
		assert!(AssetIndex::saft_nav().unwrap().is_zero());

		// `NAV/asset_price`
		let nav_asset_price = AssetIndex::relative_asset_price(ASSET_A_ID).unwrap();
		assert_eq!(nav_asset_price.price, nav / asset_price);
		assert_eq!(nav_asset_price.volume(index_token_units).unwrap(), asset_amount);
	})
}

#[test]
fn can_calculate_random_nav() {
	// generate some random accounts and assets
	let mut rng = rand::thread_rng();
	let accounts: Vec<_> = ((ACCOUNT_ID + 1)..rng.gen_range(50..100)).collect();
	let assets: Vec<_> = ((PINT_ASSET_ID + 1)..rng.gen_range(5..10)).collect();
	let prefund = 1_000;
	// make sure all are funded
	let balances = accounts
		.iter()
		.cloned()
		.flat_map(|account| assets.iter().cloned().map(move |asset| (account, asset, prefund)))
		.collect::<Vec<_>>();

	// set random prices for the assets
	MockPriceFeed::set_random_prices(assets.iter().cloned(), 1..13);

	ExtBuilder::default().with_balances(balances.clone()).build().execute_with(|| {
		// register all assets first
		for asset in assets.iter().cloned() {
			assert_ok!(AssetIndex::register_asset(
				Origin::signed(ACCOUNT_ID),
				asset,
				AssetAvailability::Liquid(MultiLocation::Null),
			));
		}

		for (account, asset, units) in balances.iter().cloned() {
			// can't deposit in an empty index
			assert_noop!(
				AssetIndex::deposit(Origin::signed(account), asset, units),
				pallet::Error::<Test>::InsufficientIndexTokens
			);
		}

		let initial_index_token_supply: Balance = 100_000_000;
		// mint some initial index token supply
		Balances::make_free_balance_be(&ACCOUNT_ID, initial_index_token_supply);
		assert_eq!(AssetIndex::index_token_issuance(), initial_index_token_supply);

		// NAV is still zero because no assets secured, meaning all index tokens are worthless
		assert!(AssetIndex::nav().unwrap().is_zero());

		// secure the index token with assets by minting them into the index' treasury
		let initial_asset_supply: Balance = 10_000_000;
		for asset in assets.iter().cloned() {
			assert_ok!(Currency::deposit(asset, &AssetIndex::treasury_account(), initial_asset_supply));
		}
		// the nav is now the combined value of all assets divided by the initial index token suply
		let total_value = AssetIndex::total_net_liquid_value().unwrap();
		let mut expected_value = 0;
		for asset in assets.iter().cloned() {
			let price = MockPriceFeed::get_price(asset).unwrap();
			let backed = price.checked_mul_int(initial_asset_supply).unwrap();
			assert_eq!(AssetIndex::net_asset_value(asset).unwrap(), backed);
			expected_value += backed;
		}
		assert_eq!(total_value, expected_value.into());
		let nav = AssetIndex::nav().unwrap();
		assert_eq!(nav, Price::checked_from_rational(expected_value, initial_index_token_supply).unwrap());

		for (account, asset, units) in balances.iter().cloned() {
			let account_index_tokens = AssetIndex::index_token_balance(&account);
			let expected_received = AssetIndex::index_token_equivalent(asset, units).unwrap();
			// deposit
			assert_ok!(AssetIndex::deposit(Origin::signed(account), asset, units));
			let received = AssetIndex::index_token_balance(&account) - account_index_tokens;
			assert_eq!(received, expected_received);
		}
	})
}

#[test]
fn deposit_works_with_user_balance() {
	new_test_ext().execute_with(|| {
		let initial_units = 1_000;
		assert_ok!(AssetIndex::add_asset(
			Origin::signed(ACCOUNT_ID),
			ASSET_A_ID,
			100,
			MultiLocation::Null,
			initial_units
		));

		let nav = AssetIndex::nav().unwrap();
		let deposit = 1_000;

		assert_noop!(
			AssetIndex::deposit(Origin::signed(ASHLEY), ASSET_A_ID, deposit),
			orml_tokens::Error::<Test>::BalanceTooLow
		);
		// deposit some funds in the account
		assert_ok!(Currency::deposit(ASSET_A_ID, &ASHLEY, deposit));
		assert_ok!(AssetIndex::deposit(Origin::signed(ASHLEY), ASSET_A_ID, deposit));
		assert_eq!(Currency::total_balance(ASSET_A_ID, &ASHLEY), 0);

		let deposit_value = MockPriceFeed::get_price(ASSET_A_ID).unwrap().checked_mul_int(deposit).unwrap();
		let received = nav.reciprocal().unwrap().saturating_mul_int(deposit_value);

		assert_eq!(AssetIndex::index_token_balance(&ASHLEY), received);
		assert_eq!(AssetIndex::index_token_issuance(), received + initial_units);
	});
}

#[test]
fn deposit_fail_for_unsupported_assets() {
	let balance = vec![(ACCOUNT_ID, UNKNOWN_ASSET_ID, 1000)];
	new_test_ext_with_balance(balance).execute_with(|| {
		assert_ok!(AssetIndex::add_asset(Origin::signed(ACCOUNT_ID), UNKNOWN_ASSET_ID, 100, MultiLocation::Null, 5));
		assert_ok!(Currency::deposit(UNKNOWN_ASSET_ID, &ASHLEY, 1_000));
		assert_noop!(
			AssetIndex::deposit(Origin::signed(ASHLEY), UNKNOWN_ASSET_ID, 100),
			pallet::Error::<Test>::UnsupportedAsset
		);
	})
}

#[test]
fn deposit_fails_on_missing_assets() {
	new_test_ext().execute_with(|| {
		assert_ok!(AssetIndex::add_asset(Origin::signed(ACCOUNT_ID), ASSET_A_ID, 100, MultiLocation::Null, 5));

		assert_noop!(
			AssetIndex::deposit(Origin::signed(ASHLEY), ASSET_A_ID, Balance::MAX),
			orml_tokens::Error::<Test>::BalanceTooLow
		);
	})
}

#[test]
fn ensure_valid_deposit_range() {
	new_test_ext().execute_with(|| {
		assert_ok!(AssetIndex::set_deposit_range(
			Origin::signed(ACCOUNT_ID),
			DepositRange { minimum: 1, maximum: Balance::MAX }
		));

		assert_noop!(
			AssetIndex::set_deposit_range(
				Origin::signed(ACCOUNT_ID),
				DepositRange { minimum: Balance::MAX, maximum: Balance::MAX }
			),
			pallet::Error::<Test>::InvalidDepositRange
		);

		assert_noop!(
			AssetIndex::set_deposit_range(
				Origin::signed(ACCOUNT_ID),
				DepositRange { minimum: Balance::MAX, maximum: Balance::MAX - 1 }
			),
			pallet::Error::<Test>::InvalidDepositRange
		);

		assert_noop!(
			AssetIndex::set_deposit_range(
				Origin::signed(ACCOUNT_ID),
				DepositRange { minimum: 0, maximum: Balance::MAX }
			),
			pallet::Error::<Test>::InvalidDepositRange
		);
	});
}

#[test]
fn deposit_fails_on_invalid_deposit_range() {
	let deposit = 1_000;
	let initial_units = 1_000;

	new_test_ext().execute_with(|| {
		assert_ok!(AssetIndex::add_asset(
			Origin::signed(ACCOUNT_ID),
			ASSET_A_ID,
			100,
			MultiLocation::Null,
			initial_units
		));

		assert_ok!(Currency::deposit(ASSET_A_ID, &ASHLEY, 100_000));

		// fund account to cover index deposits
		assert_ok!(Currency::deposit(ASSET_A_ID, &ASHLEY, deposit));

		assert_ok!(AssetIndex::deposit(Origin::signed(ASHLEY), ASSET_A_ID, deposit));

		assert_ok!(AssetIndex::set_deposit_range(
			Origin::signed(ACCOUNT_ID),
			DepositRange { minimum: Balance::MAX - 1, maximum: Balance::MAX }
		));
		assert_noop!(
			AssetIndex::deposit(Origin::signed(ASHLEY), ASSET_A_ID, deposit),
			pallet::Error::<Test>::DepositAmountBelowMinimum
		);

		assert_ok!(AssetIndex::set_deposit_range(Origin::signed(ACCOUNT_ID), DepositRange { minimum: 1, maximum: 2 }));
		assert_noop!(
			AssetIndex::deposit(Origin::signed(ASHLEY), ASSET_A_ID, deposit),
			pallet::Error::<Test>::DepositExceedsMaximum
		);
	})
}

#[test]
fn deposit_fails_on_exceeding_limit() {
	let deposit = 1_000;
	let initial_units = 1_000;

	new_test_ext().execute_with(|| {
		assert_ok!(AssetIndex::add_asset(
			Origin::signed(ACCOUNT_ID),
			ASSET_A_ID,
			100,
			MultiLocation::Null,
			initial_units
		));

		// fund account to cover index deposits
		assert_ok!(Currency::deposit(ASSET_A_ID, &ASHLEY, deposit));

		for _ in 0..50 {
			assert_ok!(AssetIndex::deposit(Origin::signed(ASHLEY), ASSET_A_ID, 1));
		}

		assert_eq!(
			AssetIndex::deposit(Origin::signed(ASHLEY), ASSET_A_ID, 1),
			Err(pallet::Error::<Test>::TooManyDeposits.into())
		);
	})
}

#[test]
fn redemption_fee_works_on_completing_withdraw() {
	let deposit = 1_000;
	let initial_units = 1_000;

	new_test_ext().execute_with(|| {
		assert_ok!(AssetIndex::add_asset(
			Origin::signed(ACCOUNT_ID),
			ASSET_A_ID,
			100,
			MultiLocation::Null,
			initial_units
		));
		assert_ok!(Currency::deposit(ASSET_A_ID, &ASHLEY, deposit));
		for _ in 0..50 {
			assert_ok!(AssetIndex::deposit(Origin::signed(ASHLEY), ASSET_A_ID, initial_units / 50));
		}

		assert_eq!(Currency::total_balance(ASSET_A_ID, &ASHLEY), 0);

		// advance the block number so that the lock expires
		let total = AssetIndex::index_token_balance(&ASHLEY);
		let current_block = frame_system::Pallet::<Test>::block_number();
		let new_block_number = LockupPeriod::get() + 1;
		frame_system::Pallet::<Test>::set_block_number(new_block_number);
		assert_ok!(AssetIndex::withdraw(Origin::signed(ASHLEY), total * 99 / 100));
		assert_ok!(AssetIndex::complete_withdraw(Origin::signed(ASHLEY)));
		assert_eq!(Currency::total_balance(ASSET_A_ID, &ASHLEY), deposit * 99 / 100);

		// ensure the redemption fee hook works
		let index_token_per_deposit = AssetIndex::index_token_equivalent(ASSET_A_ID, deposit).unwrap() / 50;
		assert_eq!(
			crate::LastRedemption::<Test>::get(),
			(new_block_number - current_block, index_token_per_deposit / 2)
		);

		// all deposits has been cleaned
		assert_eq!(crate::Deposits::<Test>::get(&ASHLEY)[0], (total / 100, 1_u64));

		// can deposit again after withdrawal
		assert_ok!(AssetIndex::deposit(Origin::signed(ASHLEY), ASSET_A_ID, 1));
	})
}

#[test]
fn can_calculate_nav() {
	new_test_ext().execute_with(|| {
		let a_units = 100;
		let b_units = 3000;
		let index_token_units = 5;
		let saft_units = 50;

		assert_ok!(AssetIndex::add_asset(
			Origin::signed(ACCOUNT_ID),
			ASSET_A_ID,
			a_units,
			MultiLocation::Null,
			index_token_units
		));
		assert_ok!(AssetIndex::add_saft(&ACCOUNT_ID, ASSET_B_ID, b_units, saft_units));

		let total_supply = AssetIndex::index_token_issuance();
		assert_eq!(
			total_supply,
			saft_units / AssetIndex::nav().unwrap().checked_mul_int(1_u128).unwrap() + index_token_units
		);

		let liquid_nav = AssetIndex::liquid_nav().unwrap();
		let saft_nav = AssetIndex::saft_nav().unwrap();
		let nav = AssetIndex::nav().unwrap();
		assert_eq!(nav, liquid_nav + saft_nav);
	})
}

#[test]
fn can_withdraw() {
	new_test_ext().execute_with(|| {
		let a_units = 100;
		let b_units = 3000;
		let a_tokens = 500;
		let b_tokens = 100;

		// create liquid assets
		assert_ok!(AssetIndex::add_asset(
			Origin::signed(ACCOUNT_ID),
			ASSET_A_ID,
			a_units,
			MultiLocation::Null,
			a_tokens
		));
		assert_ok!(AssetIndex::add_asset(
			Origin::signed(ACCOUNT_ID),
			ASSET_B_ID,
			b_units,
			MultiLocation::Null,
			b_tokens
		));

		// deposit some funds into the index from an user account
		assert_ok!(Currency::deposit(ASSET_A_ID, &ASHLEY, 1_000));
		assert_ok!(Currency::deposit(ASSET_B_ID, &ASHLEY, 2_000));
		assert_ok!(AssetIndex::deposit(Origin::signed(ASHLEY), ASSET_A_ID, 1_000));
		assert_ok!(AssetIndex::deposit(Origin::signed(ASHLEY), ASSET_B_ID, 2_000));

		assert_noop!(AssetIndex::withdraw(Origin::signed(ASHLEY), 1), pallet::Error::<Test>::MinimumRedemption);
		// try to withdraw all funds, but are locked
		assert_noop!(
			AssetIndex::withdraw(Origin::signed(ASHLEY), AssetIndex::index_token_balance(&ASHLEY)),
			pallet_balances::Error::<Test>::LiquidityRestrictions
		);

		// all index token are currently locked
		assert_eq!(pallet::LockedIndexToken::<Test>::get(&ASHLEY), AssetIndex::index_token_balance(&ASHLEY));

		// advance the block number so that the lock expires
		frame_system::Pallet::<Test>::set_block_number(LockupPeriod::get() + 1);

		// withdraw all funds
		assert_ok!(AssetIndex::withdraw(Origin::signed(ASHLEY), AssetIndex::index_token_balance(&ASHLEY)));

		let mut pending =
			pallet::PendingWithdrawals::<Test>::get(&ASHLEY).expect("pending withdrawals should be present");

		assert_eq!(pending.len(), 1);
		let pending = pending.remove(0);
		assert_eq!(pending.assets.len(), 2);
	})
}
