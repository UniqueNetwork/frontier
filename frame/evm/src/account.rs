// Copyright 2019-2022 Unique Network (Gibraltar) Ltd.
// This file is part of Unique Network.

// Unique Network is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Unique Network is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Unique Network. If not, see <http://www.gnu.org/licenses/>.

use crate::AddressMapping;
use codec::{Encode, EncodeLike, Decode, MaxEncodedLen};
use sp_core::H160;
use scale_info::{Type, TypeInfo};
use core::cmp::Ordering;
use sp_std::vec::Vec;
use sp_std::clone::Clone;
use up_evm_mapping::EvmBackwardsAddressMapping;
use frame_system::Config as FrameSystemConfig;

#[cfg(feature = "std")]
use serde::{Serialize, Deserialize};


pub trait Config: FrameSystemConfig {
	type CrossAccountId: CrossAccountId<Self::AccountId>;
	type EvmAddressMapping: AddressMapping<Self::AccountId>;
	type EvmBackwardsAddressMapping: up_evm_mapping::EvmBackwardsAddressMapping<Self::AccountId>;
}

pub trait CrossAccountId<AccountId>:
	Encode + EncodeLike + Decode + TypeInfo + MaxEncodedLen + Clone + PartialEq + Ord + core::fmt::Debug
// +
// Serialize + Deserialize<'static>
{
	fn as_sub(&self) -> &AccountId;
	fn as_eth(&self) -> &H160;

	fn from_sub(account: AccountId) -> Self;
	fn from_eth(account: H160) -> Self;

	fn conv_eq(&self, other: &Self) -> bool;
}

#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
#[derive(Encode, Decode, TypeInfo, MaxEncodedLen)]
enum BasicCrossAccountIdRepr<AccountId> {
	Substrate(AccountId),
	Ethereum(H160),
}

#[derive(PartialEq, Eq)]
pub struct BasicCrossAccountId<T: Config> {
	/// If true - then ethereum is canonical encoding
	from_ethereum: bool,
	substrate: T::AccountId,
	ethereum: H160,
}

impl<T: Config> MaxEncodedLen for BasicCrossAccountId<T> {
	fn max_encoded_len() -> usize {
		<BasicCrossAccountIdRepr<T::AccountId>>::max_encoded_len()
	}
}

impl<T: Config> TypeInfo for BasicCrossAccountId<T> {
	type Identity = Self;

	fn type_info() -> Type {
		<BasicCrossAccountIdRepr<T::AccountId>>::type_info()
	}
}

impl<T: Config> core::fmt::Debug for BasicCrossAccountId<T> {
	fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::fmt::Result {
		if self.from_ethereum {
			fmt.debug_tuple("CrossAccountId::Ethereum")
				.field(&self.ethereum)
				.finish()
		} else {
			fmt.debug_tuple("CrossAccountId::Substrate")
				.field(&self.substrate)
				.finish()
		}
	}
}

impl<T: Config> PartialOrd for BasicCrossAccountId<T> {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		Some(self.substrate.cmp(&other.substrate))
	}
}

impl<T: Config> Ord for BasicCrossAccountId<T> {
	fn cmp(&self, other: &Self) -> Ordering {
		self.partial_cmp(other)
			.expect("substrate account is total ordered")
	}
}

impl<T: Config> Clone for BasicCrossAccountId<T> {
	fn clone(&self) -> Self {
		Self {
			from_ethereum: self.from_ethereum,
			substrate: self.substrate.clone(),
			ethereum: self.ethereum,
		}
	}
}
impl<T: Config> Encode for BasicCrossAccountId<T> {
	fn encode(&self) -> Vec<u8> {
		BasicCrossAccountIdRepr::from(self.clone()).encode()
	}
}
impl<T: Config> EncodeLike for BasicCrossAccountId<T> {}
impl<T: Config> Decode for BasicCrossAccountId<T> {
	fn decode<I>(input: &mut I) -> Result<Self, codec::Error>
	where
		I: codec::Input,
	{
		Ok(BasicCrossAccountIdRepr::decode(input)?.into())
	}
}

#[cfg(feature = "std")]
impl<T> Serialize for BasicCrossAccountId<T>
where
	T: Config,
	T::AccountId: Serialize,
{
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		let repr = BasicCrossAccountIdRepr::from(self.clone());
		(&repr).serialize(serializer)
	}
}

#[cfg(feature = "std")]
impl<'de, T> Deserialize<'de> for BasicCrossAccountId<T>
where
	T: Config,
	T::AccountId: Deserialize<'de>,
{
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>,
	{
		Ok(BasicCrossAccountIdRepr::deserialize(deserializer)?.into())
	}
}

impl<T: Config> CrossAccountId<T::AccountId> for BasicCrossAccountId<T> {
	fn as_sub(&self) -> &T::AccountId {
		&self.substrate
	}
	fn as_eth(&self) -> &H160 {
		&self.ethereum
	}
	fn from_sub(substrate: T::AccountId) -> Self {
		Self {
			ethereum: T::EvmBackwardsAddressMapping::from_account_id(substrate.clone()),
			substrate,
			from_ethereum: false,
		}
	}
	fn from_eth(ethereum: H160) -> Self {
		Self {
			ethereum,
			substrate: T::EvmAddressMapping::into_account_id(ethereum),
			from_ethereum: true,
		}
	}
	fn conv_eq(&self, other: &Self) -> bool {
		if self.from_ethereum == other.from_ethereum {
			self.substrate == other.substrate && self.ethereum == other.ethereum
		} else if self.from_ethereum {
			// ethereum is canonical encoding, but we need to compare derived address
			self.substrate == other.substrate
		} else {
			self.ethereum == other.ethereum
		}
	}
}
impl<T: Config> From<BasicCrossAccountIdRepr<T::AccountId>> for BasicCrossAccountId<T> {
	fn from(repr: BasicCrossAccountIdRepr<T::AccountId>) -> Self {
		match repr {
			BasicCrossAccountIdRepr::Substrate(s) => Self::from_sub(s),
			BasicCrossAccountIdRepr::Ethereum(e) => Self::from_eth(e),
		}
	}
}
impl<T: Config> From<BasicCrossAccountId<T>> for BasicCrossAccountIdRepr<T::AccountId> {
	fn from(v: BasicCrossAccountId<T>) -> Self {
		if v.from_ethereum {
			BasicCrossAccountIdRepr::Ethereum(*v.as_eth())
		} else {
			BasicCrossAccountIdRepr::Substrate(v.as_sub().clone())
		}
	}
}