//! Type definitions for the V1 EIP-155 "native" payment scheme.
//!
//! This module defines the wire format types for native token payments
//! on EVM chains using the V1 x402 protocol.

use alloy_primitives::{Address, TxHash, U256};
use serde::{Deserialize, Serialize};
use x402_types::lit_str;
use x402_types::proto::v1;

lit_str!(NativeScheme, "native");

/// Type alias for V1 verify requests using the native EVM payment scheme.
pub type VerifyRequest = v1::VerifyRequest<PaymentPayload, PaymentRequirements>;

/// Type alias for V1 settle requests (same structure as verify requests).
pub type SettleRequest = VerifyRequest;

/// Type alias for V1 payment payloads with native EVM-specific data.
pub type PaymentPayload = v1::PaymentPayload<NativeScheme, NativePaymentPayload>;

/// Native token payment referencing an already-submitted transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativePaymentPayload {
    /// Transaction hash for the native token transfer.
    pub tx_hash: TxHash,
    /// Sender address for the transaction.
    pub from: Address,
    /// Recipient address for the transaction.
    pub to: Address,
    /// Amount transferred, in wei.
    #[serde(with = "crate::decimal_u256")]
    pub amount_wei: U256,
}

/// Type alias for V1 payment requirements with native EVM-specific types.
pub type PaymentRequirements =
    v1::PaymentRequirements<NativeScheme, U256, Address, serde_json::Value>;
