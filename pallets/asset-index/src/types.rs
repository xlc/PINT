// Copyright 2021 ChainSafe Systems
// SPDX-License-Identifier: LGPL-3.0-only

use crate::traits::MultiAssetRegistry;
use codec::FullCodec;
use frame_support::pallet_prelude::*;
use frame_support::sp_runtime::traits::{AtLeast32BitUnsigned, Convert};
use frame_support::sp_runtime::SaturatedConversion;
use pallet_asset_depository::MultiAssetDepository;
use sp_std::{
    cmp::{Eq, PartialEq},
    fmt::Debug,
    marker::PhantomData,
    prelude::*,
    result,
};
use xcm::v0::{Error as XcmError, MultiAsset, MultiLocation, Result};
use xcm_executor::{
    traits::{MatchesFungible, TransactAsset},
    Assets,
};

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
/// Defines the location of an asset
/// Liquid implies it exists on a chain somewhere in the network and
/// can be moved around
/// SAFT implies the asset is a Simple Agreement for Future Tokens and the
/// promised tokens are not able to be transferred or traded until some time
/// in the future.
pub enum AssetAvailability {
    Liquid(MultiLocation),
    Saft,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
/// A representation of some number of assets that are managed by the index
pub struct IndexAssetData<Balance> {
    pub units: Balance,
    pub availability: AssetAvailability,
}

impl<Balance> IndexAssetData<Balance> {
    pub fn new(units: Balance, availability: AssetAvailability) -> Self {
        Self {
            units,
            availability,
        }
    }
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
/// State of a single asset withdrawal on some parachain
pub enum RedemptionState {
    Initiated,
    Unbonding,
    Transferred,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
/// Represents a single asset being withdrawn
pub struct AssetWithdrawal<AssetId, Balance> {
    asset: AssetId,
    state: RedemptionState,
    units: Balance,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
/// Describes an in progress withdrawal of a collection of assets from the index
pub struct PendingRedemption<AssetId, Balance, BlockNumber> {
    initiated: BlockNumber,
    assets: Vec<AssetWithdrawal<AssetId, Balance>>,
}

/// Asset transaction errors.
enum Error {
    /// Failed to match fungible.
    FailedToMatchFungible,
    /// `MultiLocation` to `AccountId` Conversion failed.
    AccountIdConversionFailed,
    /// `CurrencyId` conversion failed.
    CurrencyIdConversionFailed,
}

impl From<Error> for XcmError {
    fn from(e: Error) -> Self {
        match e {
            Error::FailedToMatchFungible => {
                XcmError::FailedToTransactAsset("FailedToMatchFungible")
            }
            Error::AccountIdConversionFailed => {
                XcmError::FailedToTransactAsset("AccountIdConversionFailed")
            }
            Error::CurrencyIdConversionFailed => {
                XcmError::FailedToTransactAsset("CurrencyIdConversionFailed")
            }
        }
    }
}

/// The `TransactAsset` implementation, to handle deposit/withdraw for a `MultiAsset`.
pub struct MultiAssetAdapter<
    Balance,
    MultiAssets,
    AssetRegistry,
    Matcher,
    AccountId,
    AccountIdConvert,
    AssetId,
    AssetIdConvert,
>(
    PhantomData<(
        Balance,
        MultiAssets,
        AssetRegistry,
        Matcher,
        AccountId,
        AccountIdConvert,
        AssetId,
        AssetIdConvert,
    )>,
);
impl<
        Balance: AtLeast32BitUnsigned,
        MultiAssets: MultiAssetDepository<AssetId, AccountId, Balance>,
        AssetRegistry: MultiAssetRegistry<AssetId>,
        Matcher: MatchesFungible<Balance>,
        AccountId: sp_std::fmt::Debug,
        AccountIdConvert: Convert<MultiLocation, Option<AccountId>>,
        AssetId: FullCodec + Eq + PartialEq + Copy + MaybeSerializeDeserialize + Debug,
        AssetIdConvert: Convert<MultiAsset, Option<AssetId>>,
    > TransactAsset
    for MultiAssetAdapter<
        Balance,
        MultiAssets,
        AssetRegistry,
        Matcher,
        AccountId,
        AccountIdConvert,
        AssetId,
        AssetIdConvert,
    >
{
    fn deposit_asset(asset: &MultiAsset, location: &MultiLocation) -> Result {
        match (
            AccountIdConvert::convert(location.clone()),
            AssetIdConvert::convert(asset.clone()),
            Matcher::matches_fungible(asset),
        ) {
            // known asset
            (Some(who), Some(asset_id), Some(amount)) => {
                MultiAssets::deposit(&asset_id, &who, amount)
                    .map_err(|e| XcmError::FailedToTransactAsset(e.into()))
            }
            // unknown asset
            _ => Err(XcmError::FailedToTransactAsset("Unknown Asset")),
        }
    }

    fn withdraw_asset(
        asset: &MultiAsset,
        location: &MultiLocation,
    ) -> result::Result<Assets, XcmError> {
        let who = AccountIdConvert::convert(location.clone())
            .ok_or_else(|| XcmError::from(Error::AccountIdConversionFailed))?;
        let asset_id = AssetIdConvert::convert(asset.clone())
            .ok_or_else(|| XcmError::from(Error::CurrencyIdConversionFailed))?;
        let amount: Balance = Matcher::matches_fungible(&asset)
            .ok_or_else(|| XcmError::from(Error::FailedToMatchFungible))?
            .saturated_into();
        MultiAssets::withdraw(&asset_id, &who, amount)
            .map_err(|e| XcmError::FailedToTransactAsset(e.into()))?;
        Ok(asset.clone().into())
    }
}
