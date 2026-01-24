//! V1 EIP-155 "native" payment scheme implementation.
//!
//! This module implements the "native" payment scheme for EVM chains using
//! the V1 x402 protocol. It validates already-submitted native token transfers
//! by inspecting on-chain transactions.

use x402_types::scheme::X402SchemeId;

#[cfg(feature = "facilitator")]
pub mod facilitator;
#[cfg(feature = "facilitator")]
pub use facilitator::*;

pub mod types;
pub use types::*;

#[cfg(all(test, feature = "facilitator", feature = "alloy-transport"))]
mod facilitator_tests;

pub struct V1Eip155Native;

impl X402SchemeId for V1Eip155Native {
    fn x402_version(&self) -> u8 {
        1
    }

    fn namespace(&self) -> &str {
        "eip155"
    }

    fn scheme(&self) -> &str {
        NativeScheme.as_ref()
    }
}
