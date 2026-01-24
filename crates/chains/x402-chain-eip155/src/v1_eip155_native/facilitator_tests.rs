use alloy_consensus::{ReceiptEnvelope, Signed, TxEnvelope, TxLegacy};
use alloy_consensus::transaction::Recovered;
use alloy_json_rpc::{ErrorPayload, ResponsePayload};
use alloy_primitives::{Address, TxHash, TxKind, U256};
use alloy_provider::{Provider, ProviderBuilder};
use alloy_rpc_types_eth::Transaction;
use alloy_transport::mock::Asserter;
use x402_types::proto::v1::X402Version1;

use crate::chain::Eip155ChainReference;
use crate::v1_eip155_native::types::{self, NativeScheme};
use crate::v1_eip155_native::facilitator::verify_native_payment;
use x402_types::proto::PaymentVerificationError;

fn build_provider(asserter: Asserter) -> impl Provider {
    ProviderBuilder::new().connect_mocked_client(asserter)
}

fn make_tx(to: Address, value: U256) -> Transaction {
    let inner = TxLegacy {
        chain_id: None,
        nonce: 1,
        gas_price: 1,
        gas_limit: 21000,
        to: TxKind::Call(to),
        value,
        input: alloy_primitives::Bytes::new(),
    };
    let signature = alloy_primitives::Signature::test_signature();
    let signed = Signed::new_unhashed(inner, signature);
    let envelope = TxEnvelope::Legacy(signed);
    let recovered = Recovered::new_unchecked(envelope, Address::ZERO);
    Transaction {
        inner: recovered,
        block_hash: None,
        block_number: None,
        transaction_index: None,
        effective_gas_price: None,
    }
}

fn make_receipt(from: Address, success: bool) -> alloy_rpc_types_eth::TransactionReceipt {
    let receipt = alloy_consensus::Receipt {
        status: success.into(),
        cumulative_gas_used: 1,
        logs: Vec::new(),
    };
    let with_bloom = receipt.with_bloom();
    alloy_rpc_types_eth::TransactionReceipt {
        inner: ReceiptEnvelope::Legacy(with_bloom),
        transaction_hash: TxHash::ZERO,
        transaction_index: None,
        block_hash: None,
        block_number: None,
        gas_used: 1,
        effective_gas_price: 1,
        blob_gas_used: None,
        blob_gas_price: None,
        from,
        to: None,
        contract_address: None,
    }
}

fn base_payload(to: Address, from: Address, amount: U256) -> types::PaymentPayload {
    types::PaymentPayload {
        x402_version: X402Version1,
        scheme: NativeScheme,
        network: "base".to_string(),
        payload: types::NativePaymentPayload {
            tx_hash: TxHash::ZERO,
            from,
            to,
            amount_wei: amount,
        },
    }
}

fn base_requirements(to: Address, amount: U256) -> types::PaymentRequirements {
    types::PaymentRequirements {
        scheme: NativeScheme,
        network: "base".to_string(),
        max_amount_required: amount,
        resource: "https://example.com/resource".parse().expect("resource url"),
        description: "native".to_string(),
        mime_type: "text/plain".to_string(),
        output_schema: None,
        pay_to: to,
        max_timeout_seconds: 300,
        asset: Address::ZERO,
        extra: Some(serde_json::Value::Null),
    }
}

#[tokio::test]
async fn verify_native_payment_rejects_payload_amount_below_requirement() {
    let asserter = Asserter::new();
    let provider = build_provider(asserter);
    let chain = Eip155ChainReference::new(8453);
    let to = Address::from([0x11u8; 20]);
    let from = Address::from([0x22u8; 20]);

    let payload = base_payload(to, from, U256::from(5));
    let requirements = base_requirements(to, U256::from(10));

    let result = verify_native_payment(&provider, &chain, &payload, &requirements).await;
    assert!(matches!(result, Err(PaymentVerificationError::InvalidPaymentAmount)));
}

#[tokio::test]
async fn verify_native_payment_errors_when_tx_missing() {
    let asserter = Asserter::new();
    asserter.push_success(&Option::<Transaction>::None);
    let provider = build_provider(asserter);
    let chain = Eip155ChainReference::new(8453);
    let to = Address::from([0x11u8; 20]);
    let from = Address::from([0x22u8; 20]);

    let payload = base_payload(to, from, U256::from(10));
    let requirements = base_requirements(to, U256::from(10));

    let result = verify_native_payment(&provider, &chain, &payload, &requirements).await;
    assert!(matches!(result, Err(PaymentVerificationError::InvalidFormat(_))));
}

#[tokio::test]
async fn verify_native_payment_errors_when_tx_value_insufficient() {
    let asserter = Asserter::new();
    let tx = make_tx(Address::from([0x11u8; 20]), U256::from(5));
    asserter.push_success(&Some(tx));
    let provider = build_provider(asserter);
    let chain = Eip155ChainReference::new(8453);
    let to = Address::from([0x11u8; 20]);
    let from = Address::from([0x22u8; 20]);

    let payload = base_payload(to, from, U256::from(10));
    let requirements = base_requirements(to, U256::from(10));

    let result = verify_native_payment(&provider, &chain, &payload, &requirements).await;
    assert!(matches!(result, Err(PaymentVerificationError::InvalidPaymentAmount)));
}

#[tokio::test]
async fn verify_native_payment_errors_when_payload_amount_mismatch() {
    let asserter = Asserter::new();
    let to = Address::from([0x11u8; 20]);
    let from = Address::from([0x22u8; 20]);
    let tx = make_tx(to, U256::from(10));
    let receipt = make_receipt(from, true);
    asserter.push_success(&Some(tx));
    asserter.push_success(&Some(receipt));
    let provider = build_provider(asserter);
    let chain = Eip155ChainReference::new(8453);

    let payload = base_payload(to, from, U256::from(12));
    let requirements = base_requirements(to, U256::from(10));

    let result = verify_native_payment(&provider, &chain, &payload, &requirements).await;
    assert!(matches!(result, Err(PaymentVerificationError::InvalidPaymentAmount)));
}

#[tokio::test]
async fn verify_native_payment_errors_when_receipt_reverted() {
    let asserter = Asserter::new();
    let to = Address::from([0x11u8; 20]);
    let from = Address::from([0x22u8; 20]);
    let tx = make_tx(to, U256::from(10));
    let receipt = make_receipt(from, false);
    asserter.push_success(&Some(tx));
    asserter.push_success(&Some(receipt));
    let provider = build_provider(asserter);
    let chain = Eip155ChainReference::new(8453);

    let payload = base_payload(to, from, U256::from(10));
    let requirements = base_requirements(to, U256::from(10));

    let result = verify_native_payment(&provider, &chain, &payload, &requirements).await;
    assert!(matches!(result, Err(PaymentVerificationError::TransactionSimulation(_))));
}

#[tokio::test]
async fn verify_native_payment_errors_when_receipt_sender_mismatch() {
    let asserter = Asserter::new();
    let to = Address::from([0x11u8; 20]);
    let from = Address::from([0x22u8; 20]);
    let tx = make_tx(to, U256::from(10));
    let receipt = make_receipt(Address::from([0x33u8; 20]), true);
    asserter.push_success(&Some(tx));
    asserter.push_success(&Some(receipt));
    let provider = build_provider(asserter);
    let chain = Eip155ChainReference::new(8453);

    let payload = base_payload(to, from, U256::from(10));
    let requirements = base_requirements(to, U256::from(10));

    let result = verify_native_payment(&provider, &chain, &payload, &requirements).await;
    assert!(matches!(result, Err(PaymentVerificationError::InvalidSignature(_))));
}

#[tokio::test]
async fn verify_native_payment_returns_payer_when_valid() {
    let asserter = Asserter::new();
    let to = Address::from([0x11u8; 20]);
    let from = Address::from([0x22u8; 20]);
    let tx = make_tx(to, U256::from(10));
    let receipt = make_receipt(from, true);
    asserter.push_success(&Some(tx));
    asserter.push_success(&Some(receipt));
    let provider = build_provider(asserter);
    let chain = Eip155ChainReference::new(8453);

    let payload = base_payload(to, from, U256::from(10));
    let requirements = base_requirements(to, U256::from(10));

    let result = verify_native_payment(&provider, &chain, &payload, &requirements).await;
    assert_eq!(result.expect("valid payment"), from);
}

#[tokio::test]
async fn verify_native_payment_returns_transport_simulation_error() {
    let asserter = Asserter::new();
    asserter.push(ResponsePayload::Failure(ErrorPayload::internal_error()));
    let provider = build_provider(asserter);
    let chain = Eip155ChainReference::new(8453);
    let to = Address::from([0x11u8; 20]);
    let from = Address::from([0x22u8; 20]);

    let payload = base_payload(to, from, U256::from(10));
    let requirements = base_requirements(to, U256::from(10));

    let result = verify_native_payment(&provider, &chain, &payload, &requirements).await;
    assert!(matches!(result, Err(PaymentVerificationError::TransactionSimulation(_))));
}

#[tokio::test]
async fn verify_native_payment_errors_on_wrong_network() {
    let asserter = Asserter::new();
    let provider = build_provider(asserter);
    let chain = Eip155ChainReference::new(8453);
    let to = Address::from([0x11u8; 20]);
    let from = Address::from([0x22u8; 20]);

    let mut payload = base_payload(to, from, U256::from(10));
    payload.network = "unknown".to_string();
    let requirements = base_requirements(to, U256::from(10));

    let result = verify_native_payment(&provider, &chain, &payload, &requirements).await;
    assert!(matches!(result, Err(PaymentVerificationError::UnsupportedChain)));
}

#[tokio::test]
async fn verify_native_payment_errors_on_network_mismatch() {
    let asserter = Asserter::new();
    let provider = build_provider(asserter);
    let chain = Eip155ChainReference::new(84532);
    let to = Address::from([0x11u8; 20]);
    let from = Address::from([0x22u8; 20]);

    let payload = base_payload(to, from, U256::from(10));
    let requirements = base_requirements(to, U256::from(10));

    let result = verify_native_payment(&provider, &chain, &payload, &requirements).await;
    assert!(matches!(result, Err(PaymentVerificationError::ChainIdMismatch)));
}
