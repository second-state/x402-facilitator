//! Facilitator-side payment verification and settlement for V1 EIP-155 native scheme.
//!
//! This module validates already-submitted native token payments by inspecting
//! on-chain transactions and receipts.

use alloy_primitives::Address;
use alloy_provider::Provider;
use std::collections::HashMap;
use x402_types::chain::{ChainId, ChainProviderOps};
use x402_types::proto;
use x402_types::proto::{PaymentVerificationError, v1};
use x402_types::scheme::{
    X402SchemeFacilitator, X402SchemeFacilitatorBuilder, X402SchemeFacilitatorError,
};

use crate::V1Eip155Native;
use crate::chain::{Eip155ChainReference, Eip155MetaTransactionProvider};
use crate::v1_eip155_native::types::{self, NativePaymentPayload, NativeScheme};

use alloy_consensus::Transaction as _;

impl<P> X402SchemeFacilitatorBuilder<P> for V1Eip155Native
where
    P: Eip155MetaTransactionProvider + ChainProviderOps + Send + Sync + 'static,
{
    fn build(
        &self,
        provider: P,
        _config: Option<serde_json::Value>,
    ) -> Result<Box<dyn X402SchemeFacilitator>, Box<dyn std::error::Error>> {
        Ok(Box::new(V1Eip155NativeFacilitator::new(provider)))
    }
}

/// Facilitator for V1 EIP-155 native scheme payments.
pub struct V1Eip155NativeFacilitator<P> {
    provider: P,
}

impl<P> V1Eip155NativeFacilitator<P> {
    /// Creates a new V1 EIP-155 native scheme facilitator with the given provider.
    pub fn new(provider: P) -> Self {
        Self { provider }
    }
}

#[async_trait::async_trait]
impl<P> X402SchemeFacilitator for V1Eip155NativeFacilitator<P>
where
    P: Eip155MetaTransactionProvider + ChainProviderOps + Send + Sync,
    P::Inner: Provider,
{
    async fn verify(
        &self,
        request: &proto::VerifyRequest,
    ) -> Result<proto::VerifyResponse, X402SchemeFacilitatorError> {
        let request = types::VerifyRequest::from_proto(request.clone())?;
        let payload = &request.payment_payload;
        let requirements = &request.payment_requirements;
        let payer = verify_native_payment(
            self.provider.inner(),
            self.provider.chain(),
            payload,
            requirements,
        )
        .await?;
        Ok(v1::VerifyResponse::valid(payer.to_string()).into())
    }

    async fn settle(
        &self,
        request: &proto::SettleRequest,
    ) -> Result<proto::SettleResponse, X402SchemeFacilitatorError> {
        let request = types::SettleRequest::from_proto(request.clone())?;
        let payload = &request.payment_payload;
        let requirements = &request.payment_requirements;
        let payer = verify_native_payment(
            self.provider.inner(),
            self.provider.chain(),
            payload,
            requirements,
        )
        .await?;

        Ok(v1::SettleResponse::Success {
            payer: payer.to_string(),
            transaction: payload.payload.tx_hash.to_string(),
            network: requirements.network.clone(),
        }
        .into())
    }

    async fn supported(&self) -> Result<proto::SupportedResponse, X402SchemeFacilitatorError> {
        let chain_id = self.provider.chain_id();
        let kinds = {
            let mut kinds = Vec::with_capacity(1);
            let network = chain_id.as_network_name();
            if let Some(network) = network {
                kinds.push(proto::SupportedPaymentKind {
                    x402_version: v1::X402Version1.into(),
                    scheme: NativeScheme.to_string(),
                    network: network.to_string(),
                    extra: None,
                });
            }
            kinds
        };
        let signers = {
            let mut signers = HashMap::with_capacity(1);
            signers.insert(chain_id, self.provider.signer_addresses());
            signers
        };
        Ok(proto::SupportedResponse {
            kinds,
            extensions: Vec::new(),
            signers,
        })
    }
}

#[cfg_attr(feature = "telemetry", tracing::instrument(skip_all, err))]
pub(crate) async fn verify_native_payment<P: Provider>(
    provider: &P,
    chain: &Eip155ChainReference,
    payload: &types::PaymentPayload,
    requirements: &types::PaymentRequirements,
) -> Result<Address, PaymentVerificationError> {
    let chain_id: ChainId = chain.into();
    let payload_chain_id = ChainId::from_network_name(&payload.network)
        .ok_or(PaymentVerificationError::UnsupportedChain)?;
    if payload_chain_id != chain_id {
        return Err(PaymentVerificationError::ChainIdMismatch);
    }
    let requirements_chain_id = ChainId::from_network_name(&requirements.network)
        .ok_or(PaymentVerificationError::UnsupportedChain)?;
    if requirements_chain_id != chain_id {
        return Err(PaymentVerificationError::ChainIdMismatch);
    }

    assert_payload_matches_request_payload(&payload.payload, requirements)?;

    let tx_hash = payload.payload.tx_hash;
    let tx = provider
        .get_transaction_by_hash(tx_hash)
        .await
        .map_err(|e| PaymentVerificationError::TransactionSimulation(e.to_string()))?
        .ok_or_else(|| PaymentVerificationError::InvalidFormat("transaction not found".into()))?;

    let expected_to = requirements.pay_to;
    let tx_to = tx
        .to()
        .ok_or_else(|| PaymentVerificationError::InvalidFormat("missing recipient".into()))?;
    if tx_to != expected_to {
        return Err(PaymentVerificationError::RecipientMismatch);
    }

    let amount_required = requirements.max_amount_required;
    let tx_value = tx.value();
    if tx_value < amount_required || tx_value != payload.payload.amount_wei {
        return Err(PaymentVerificationError::InvalidPaymentAmount);
    }

    let receipt = provider
        .get_transaction_receipt(tx_hash)
        .await
        .map_err(|e| PaymentVerificationError::TransactionSimulation(e.to_string()))?
        .ok_or_else(|| PaymentVerificationError::InvalidFormat("transaction not confirmed".into()))?;
    if !receipt.status() {
        return Err(PaymentVerificationError::TransactionSimulation(
            "transaction reverted".into(),
        ));
    }

    let expected_from = payload.payload.from;
    let receipt_from = receipt.from;
    if receipt_from != expected_from {
        return Err(PaymentVerificationError::InvalidSignature(format!(
            "transaction sender mismatch: expected {}, got {}",
            expected_from, receipt_from
        )));
    }

    Ok(expected_from)
}

fn assert_payload_matches_request_payload(
    payload: &NativePaymentPayload,
    requirements: &types::PaymentRequirements,
) -> Result<(), PaymentVerificationError> {
    if payload.to != requirements.pay_to {
        return Err(PaymentVerificationError::RecipientMismatch);
    }
    if payload.amount_wei < requirements.max_amount_required {
        return Err(PaymentVerificationError::InvalidPaymentAmount);
    }
    Ok(())
}
