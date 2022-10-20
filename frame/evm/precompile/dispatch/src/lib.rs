// SPDX-License-Identifier: Apache-2.0
// This file is part of Frontier.
//
// Copyright (c) 2020-2022 Parity Technologies (UK) Ltd.
//
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

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use core::marker::PhantomData;
use fp_evm::{
	ExitError, ExitSucceed, Precompile, PrecompileFailure, PrecompileHandle, PrecompileOutput,
	PrecompileResult,
};
use frame_support::{
	codec::Decode,
	dispatch::{DispatchClass, Dispatchable, GetDispatchInfo, Pays, PostDispatchInfo},
};
use pallet_evm::{AddressMapping, GasWeightMapping};

pub struct Dispatch<T> {
	_marker: PhantomData<T>,
}

impl<T> Precompile for Dispatch<T>
where
	T: pallet_evm::Config,
	T::RuntimeCall: Dispatchable<PostInfo = PostDispatchInfo> + GetDispatchInfo + Decode,
	<T::RuntimeCall as Dispatchable>::RuntimeOrigin: From<Option<T::AccountId>>,
{
	fn execute(handle: &mut impl PrecompileHandle) -> PrecompileResult {
		let input = handle.input();
		let target_gas = handle.gas_limit();
		let context = handle.context();

		let call = T::RuntimeCall::decode(&mut &*input).map_err(|_| PrecompileFailure::Error {
			exit_status: ExitError::Other("decode failed".into()),
		})?;
		let info = call.get_dispatch_info();

		let valid_call = info.pays_fee == Pays::Yes && info.class == DispatchClass::Normal;
		if !valid_call {
			return Err(ExitError::Other("invalid call".into()).into());
		}

		if let Some(gas) = target_gas {
			let valid_weight = info.weight.all_lt(T::GasWeightMapping::gas_to_weight(gas));
			if !valid_weight {
				return Err(ExitError::OutOfGas.into());
			}
		}

		let origin = T::AddressMapping::into_account_id(context.caller);

		match call.dispatch(Some(origin).into()) {
			Ok(post_info) => {
				let cost = T::GasWeightMapping::weight_to_gas(
					post_info.actual_weight.unwrap_or(info.weight),
				);

				handle.record_cost(cost)?;

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Stopped,
					output: Default::default(),
				})
			}
			Err(_) => Err(ExitError::Other("dispatch execution failed".into()).into()),
		}
	}
}
