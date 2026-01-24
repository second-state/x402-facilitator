//! Type definitions for the V1 EIP-155 "exact" payment scheme.
//!
//! This module defines the wire format types for ERC-3009 based payments
//! on EVM chains using the V1 x402 protocol.

use alloy_primitives::{Address, Bytes, B256, U256};
use serde::{Deserialize, Serialize};
use x402_types::lit_str;
use x402_types::proto::v1;
use x402_types::timestamp::UnixTimestamp;

#[cfg(any(feature = "facilitator", feature = "client"))]
use alloy_sol_types::sol;

lit_str!(ExactScheme, "exact");

/// Type alias for V1 verify requests using the exact EVM payment scheme.
pub type VerifyRequest = v1::VerifyRequest<PaymentPayload, PaymentRequirements>;

/// Type alias for V1 settle requests (same structure as verify requests).
pub type SettleRequest = VerifyRequest;

/// Type alias for V1 payment payloads with EVM-specific data.
pub type PaymentPayload = v1::PaymentPayload<ExactScheme, ExactEvmPayload>;

/// Full payload required to authorize an ERC-3009 transfer.
///
/// This struct contains both the EIP-712 signature and the structured authorization
/// data that was signed. Together, they provide everything needed to execute a
/// `transferWithAuthorization` call on an ERC-3009 compliant token contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Erc3009Payload {
    /// The cryptographic signature authorizing the transfer.
    ///
    /// This can be:
    /// - An EOA signature (64-65 bytes, split into r, s, v components)
    /// - An EIP-1271 signature (arbitrary length, validated by contract)
    /// - An EIP-6492 signature (wrapped with deployment data and magic suffix)
    pub signature: Bytes,

    /// The structured authorization data that was signed.
    pub authorization: Erc3009Authorization,
}

/// EIP-712 structured data for ERC-3009 transfer authorization.
///
/// This struct defines the parameters of a `transferWithAuthorization` call:
/// who can transfer tokens, to whom, how much, and during what time window.
/// The struct is signed using EIP-712 typed data signing.
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Erc3009Authorization {
    /// The address authorizing the transfer (token owner).
    pub from: Address,

    /// The recipient address for the transfer.
    pub to: Address,

    /// The amount of tokens to transfer (in token's smallest unit).
    #[serde(with = "crate::decimal_u256")]
    pub value: U256,

    /// The authorization is not valid before this timestamp (inclusive).
    pub valid_after: UnixTimestamp,

    /// The authorization expires at this timestamp (exclusive).
    pub valid_before: UnixTimestamp,

    /// A unique 32-byte nonce to prevent replay attacks.
    pub nonce: B256,
}

/// EIP-2612 permit authorization data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Eip2612PermitPayload {
    pub owner: Address,
    pub spender: Address,
    #[serde(with = "crate::decimal_u256")]
    pub value: U256,
    pub deadline: UnixTimestamp,
    pub nonce: u64,
    pub signature: Bytes,
}

/// EIP-2612 transfer data for `transferFrom`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Eip2612TransferPayload {
    pub from: Address,
    pub to: Address,
    #[serde(with = "crate::decimal_u256")]
    pub amount: U256,
}

/// EIP-2612 payload: permit + transfer for permit-based payments.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Eip2612Payload {
    pub permit: Eip2612PermitPayload,
    pub transfer: Eip2612TransferPayload,
}

/// EVM payment payload supporting both ERC-3009 and EIP-2612 standards.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ExactEvmPayload {
    /// ERC-3009: Single-step transferWithAuthorization.
    Erc3009(Erc3009Payload),
    /// EIP-2612: Two-step permit + transferFrom.
    Eip2612(Eip2612Payload),
}

/// Backwards-compatible alias for EIP-3009 payloads used in V2 types.
pub type Eip3009Payload = Erc3009Payload;

/// Type alias for V1 payment requirements with EVM-specific types.
pub type PaymentRequirements =
    v1::PaymentRequirements<ExactScheme, U256, Address, PaymentRequirementsExtra>;

/// Extra EIP-712 domain parameters for token contracts.
///
/// Some token contracts require specific `name` and `version` values in their
/// EIP-712 domain for signature verification. This struct allows servers to
/// specify these values in the payment requirements, avoiding the need for
/// the facilitator to query them from the contract.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentRequirementsExtra {
    /// The token name as used in the EIP-712 domain.
    pub name: String,

    /// The token version as used in the EIP-712 domain.
    pub version: String,
}

#[cfg(any(feature = "facilitator", feature = "client"))]
sol!(
    /// Solidity-compatible struct definition for ERC-3009 `transferWithAuthorization`.
    ///
    /// This matches the EIP-3009 format used in EIP-712 typed data:
    /// it defines the authorization to transfer tokens from `from` to `to`
    /// for a specific `value`, valid only between `validAfter` and `validBefore`
    /// and identified by a unique `nonce`.
    ///
    /// This struct is primarily used to reconstruct the typed data domain/message
    /// when verifying a client's signature.
    #[derive(Serialize, Deserialize)]
    struct TransferWithAuthorization {
        address from;
        address to;
        uint256 value;
        uint256 validAfter;
        uint256 validBefore;
        bytes32 nonce;
    }
);
