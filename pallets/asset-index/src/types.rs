// Copyright 2021 ChainSafe Systems
// SPDX-License-Identifier: LGPL-3.0-only

use codec::{Decode, Encode};
use frame_support::{
	sp_runtime::{traits::Zero, FixedU128, RuntimeDebug},
	sp_std::vec::Vec,
};
use pallet_price_feed::AssetPricePair;
use xcm::opaque::v0::MultiLocation;

/// Abstraction over the lock of minted index token that are locked up for
/// `LockupPeriod`
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub struct IndexTokenLock<BlockNumber, Balance> {
	/// Locked amount of index token.
	pub locked: Balance,
	/// The block when the locked index token can be unlocked.
	pub end_block: BlockNumber,
}

/// Defines the location of an asset
/// Liquid implies it exists on a chain somewhere in the network and
/// can be moved around
/// SAFT implies the asset is a Simple Agreement for Future Tokens and the
/// promised tokens are not able to be transferred or traded until some time
/// in the future.
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub enum AssetAvailability {
	Liquid(MultiLocation),
	Saft,
}

impl AssetAvailability {
	/// Whether this asset data represents a liquid asset
	pub fn is_liquid(&self) -> bool {
		matches!(self, AssetAvailability::Liquid(_))
	}

	/// Whether this asset data represents a SAFT
	pub fn is_saft(&self) -> bool {
		matches!(self, AssetAvailability::Saft)
	}
}

impl From<MultiLocation> for AssetAvailability {
	fn from(location: MultiLocation) -> Self {
		AssetAvailability::Liquid(location)
	}
}

/// Metadata for an asset
#[derive(PartialEq, Eq, Clone, Default, Encode, Decode, RuntimeDebug)]
pub struct AssetMetadata<BoundedString> {
	pub name: BoundedString,
	pub symbol: BoundedString,
	pub decimals: u8,
}

/// State of a single asset withdrawal on some parachain
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub enum RedemptionState {
	/// Requested, but unbonding failed, either because the corresponding xcm
	/// unbonding failed to execute or because there is nothing to unbond and
	/// the minimum remote stash balance is exhausted.
	Initiated,
	/// Unbonding was successful due to:
	///   - the asset does not support staking.
	///   - the current parachain's stash account is liquid enough to cover the withdrawal after the
	///     redemption period without falling below the configured minimum stash balance threshold.
	///   - xcm unbonding call was sent successfully.
	Unbonding,
	/// This is a intermediary state in which the it's attempted to transfer the
	/// units to the LP's account
	Transferring,
	/// Successfully transferred the units to LP's account, the
	/// `AssetWithdrawal` has thus been completed.
	Withdrawn,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
/// Represents a single asset being withdrawn
pub struct AssetWithdrawal<AssetId, Balance> {
	/// The identifier of the asset
	pub asset: AssetId,
	/// The state in which the redemption process currently is.
	pub state: RedemptionState,
	/// The amount of asset units about to be transferred to the LP.
	pub units: Balance,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
/// Describes an in progress withdrawal of a collection of assets from the index
pub struct PendingRedemption<AssetId, Balance, BlockNumber> {
	/// When the redemption process is over
	pub end_block: BlockNumber,
	/// All the withdrawals resulted from the redemption
	pub assets: Vec<AssetWithdrawal<AssetId, Balance>>,
}

/// Represents the total volume of all assets measured in index token based on
/// the price and the total amount of asset units that are in the index and
/// tracks the accumulated volume
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub struct AssetsVolume<AssetId, Balance> {
	/// All `AssetVolume`
	pub volumes: Vec<AssetVolume<AssetId, Balance>>,
	/// The accumulated volume of all `volumes`
	pub total_volume: Balance,
}

/// Represents the total volume of an asset measured in index token based on the
/// price and the total amount of asset units that are in the index
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub struct AssetVolume<AssetId, Balance> {
	/// The current price pair for `PINT/asset`
	pub price: AssetPricePair<AssetId>,
	/// The amount of `PINT` equivalent all `asset` units are worth based on the
	/// `price`
	pub pint_volume: Balance,
}

impl<AssetId, Balance> AssetVolume<AssetId, Balance> {
	pub fn new(price: AssetPricePair<AssetId>, pint_volume: Balance) -> Self {
		Self { price, pint_volume }
	}
}

/// Represents the distribution of assets in the index. For each asset, the
/// corresponding ratio was determined based on the total volume and the
/// equivalent asset volume, which was determined by the price.
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub struct AssetsDistribution<AssetId, Balance> {
	/// The total amount of index token
	pub total_pint: Balance,
	/// All assets and their share of total volume
	pub asset_shares: Vec<(AssetVolume<AssetId, Balance>, FixedU128)>,
}

/// Represents the redemption of a given pint amount based on the
/// `AssetDistribution`.
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub struct AssetRedemption<AssetId, Balance> {
	/// All the assets together with their redeemed amount
	pub asset_amounts: Vec<(AssetId, Balance)>,
	/// The total amount of redeemed pint
	pub redeemed_pint: Balance,
}

impl<AssetId, Balance: Zero> Default for AssetRedemption<AssetId, Balance> {
	fn default() -> Self {
		Self { asset_amounts: Vec::new(), redeemed_pint: Balance::zero() }
	}
}
