// This file is part of Frontier.

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

use frame_support::dispatch::{DispatchInfo, GetDispatchInfo};
use scale_codec::Encode;
use sp_runtime::{
	traits::{
		self, AsTransactionAuthorizedOrigin, DispatchInfoOf, DispatchTransaction, Dispatchable, MaybeDisplay, Member, PostDispatchInfoOf, TransactionExtension, ValidateUnsigned
	},
	transaction_validity::{
		InvalidTransaction, TransactionSource, TransactionValidity, TransactionValidityError,
	},
	RuntimeDebug,
};

use crate::SelfContainedCall;

#[derive(Clone, Eq, PartialEq, RuntimeDebug)]
pub enum CheckedFormat<AccountId, Extension, SelfContainedSignedInfo> {
	Bare,
	Signed(AccountId, Extension),
	General(Extension),
	SelfContained(SelfContainedSignedInfo),
}

/// Definition of something that the external world might want to say; its
/// existence implies that it has been checked and is good, particularly with
/// regards to the signature.
#[derive(Clone, Eq, PartialEq, RuntimeDebug)]
pub struct CheckedExtrinsic<AccountId, Call, Extension, SelfContainedSignedInfo> {
	/// Who this purports to be from and the number of extrinsics have come before
	/// from the same signer, if anyone (note this is not a signature).
	pub format: CheckedFormat<AccountId, Extension, SelfContainedSignedInfo>,

	/// The function that should be called.
	pub function: Call,
}

impl<AccountId, Call: GetDispatchInfo, Extension, SelfContainedSignedInfo> GetDispatchInfo
	for CheckedExtrinsic<AccountId, Call, Extension, SelfContainedSignedInfo>
{
	fn get_dispatch_info(&self) -> DispatchInfo {
		self.function.get_dispatch_info()
	}
}

impl<AccountId, Call, Extension, SelfContainedSignedInfo, RuntimeOrigin> traits::Applyable
	for CheckedExtrinsic<AccountId, Call, Extension, SelfContainedSignedInfo>
where
	AccountId: Member + MaybeDisplay,
	Call: Member
		+ Dispatchable<RuntimeOrigin = RuntimeOrigin>
		+ Encode
		+ SelfContainedCall<SignedInfo = SelfContainedSignedInfo>,
	Extension: TransactionExtension<Call>,
	RuntimeOrigin: From<Option<AccountId>> + AsTransactionAuthorizedOrigin,
	SelfContainedSignedInfo: Send + Sync + 'static,
{
	type Call = Call;

	fn validate<I: ValidateUnsigned<Call = Self::Call>>(
		&self,
		source: TransactionSource,
		info: &DispatchInfoOf<Self::Call>,
		len: usize,
	) -> TransactionValidity {
		match &self.format {
			CheckedFormat::Bare => {
				let inherent_validation = I::validate_unsigned(source, &self.function)?;
				#[allow(deprecated)]
				let legacy_validation = Extension::bare_validate(&self.function, info, len)?;
				Ok(legacy_validation.combine_with(inherent_validation))
			},
			CheckedFormat::Signed(ref signer, ref extension) => {
				let origin = Some(signer.clone()).into();
				extension.validate_only(origin, &self.function, info, len).map(|x| x.0)
			},
			CheckedFormat::General(ref extension) =>
				extension.validate_only(None.into(), &self.function, info, len).map(|x| x.0),
			CheckedFormat::SelfContained(signed_info) => self
				.function
				.validate_self_contained(signed_info, info, len)
				.ok_or(TransactionValidityError::Invalid(
					InvalidTransaction::BadProof,
				))?,
		}
	}

	fn apply<I: ValidateUnsigned<Call = Self::Call>>(
		self,
		info: &DispatchInfoOf<Self::Call>,
		len: usize,
	) -> sp_runtime::ApplyExtrinsicResultWithInfo<PostDispatchInfoOf<Self::Call>> {
		match self.format {
			CheckedFormat::Bare => {
				I::pre_dispatch(&self.function)?;
				// TODO: Separate logic from `TransactionExtension` into a new `InherentExtension`
				// interface.
				Extension::bare_validate_and_prepare(&self.function, info, len)?;
				let res = self.function.dispatch(None.into());
				let mut post_info = res.unwrap_or_else(|err| err.post_info);
				let pd_res = res.map(|_| ()).map_err(|e| e.error);
				// TODO: Separate logic from `TransactionExtension` into a new `InherentExtension`
				// interface.
				Extension::bare_post_dispatch(info, &mut post_info, len, &pd_res)?;
				Ok(res)
			},
			CheckedFormat::Signed(signer, extension) =>
				extension.dispatch_transaction(Some(signer).into(), self.function, info, len),
			CheckedFormat::General(extension) =>
				extension.dispatch_transaction(None.into(), self.function, info, len),
			CheckedFormat::SelfContained(signed_info) => {
				// If pre-dispatch fail, the block must be considered invalid
				self.function
					.pre_dispatch_self_contained(&signed_info, info, len)
					.ok_or(TransactionValidityError::Invalid(
						InvalidTransaction::BadProof,
					))??;
				let res = self.function.apply_self_contained(signed_info).ok_or(
					TransactionValidityError::Invalid(InvalidTransaction::BadProof),
				)?;
				let mut post_info = match res {
					Ok(info) => info,
					Err(err) => err.post_info,
				};
				Extension::bare_post_dispatch(
					info,
					&mut post_info,
					len,
					&res.map(|_| ()).map_err(|e| e.error),
				)?;
				Ok(res)
			}
		}
	}
}
