//! Example of signing, encoding and sending a raw transaction using a wallet.
//!
//! This module provides functionality to create and sign Ethereum transactions
//! for transferring ETH between addresses. It handles transaction building,
//! signing with private keys, and preparing transactions for network broadcast.

use alloy_network::{eip2718::Encodable2718, EthereumWallet, TransactionBuilder};
use alloy_primitives::{Address, Bytes, ChainId, U256};
use alloy_rpc_types_eth::TransactionRequest;
use alloy_signer_local::PrivateKeySigner;
use eyre::Result;
use std::str::FromStr;

/// Creates and signs a transfer transaction from a private key to a recipient address.
///
/// This function builds a complete Ethereum transaction with the following parameters:
/// - Sender address derived from the private key
/// - Recipient address specified as a string
/// - Chain ID for network identification
/// - Amount to transfer in wei
/// - Nonce for transaction ordering
///
/// The transaction is configured with standard gas settings:
/// - Gas limit: 21,000 (standard ETH transfer)
/// - Max priority fee: 1 Gwei
/// - Max fee: 20 Gwei
///
/// # Arguments
///
/// * `signer_privkey` - The private key of the sender (byte slice)
/// * `recipient` - The recipient's Ethereum address (hex string)
/// * `chain_id` - The chain ID of the target network
/// * `gwei_amount` - The amount to transfer in wei
/// * `nonce` - The transaction nonce for the sender
///
/// # Returns
///
/// Returns a `Result` containing the raw transaction bytes if successful,
/// or an error if the transaction creation fails.
///
/// # Errors
///
/// This function will return an error if:
/// - The recipient address is invalid
/// - The private key is malformed
/// - The transaction building process fails
///
/// # Example
///
/// ```rust
/// use testing::transactions::create_transfer_transaction;
///
/// #[tokio::main]
/// async fn main() -> eyre::Result<()> {
///     let private_key = [0u8; 32]; // Your private key bytes
///     let raw_tx = create_transfer_transaction(
///         &private_key,
///         "0x456...", // recipient address
///         1,          // chain ID (mainnet)
///         1_000_000_000_000_000_000, // 1 ETH in wei
///         0           // nonce
///     ).await?;
///     
///     // raw_tx can now be sent via RPC client
///     Ok(())
/// }
/// ```
pub async fn create_transfer_transaction(
    signer_privkey: &[u8],
    recipient: &str,
    chain_id: ChainId,
    gwei_amount: u64,
    nonce: u64,
) -> Result<Bytes> {
    // Parse the recipient address from string to Address type
    let recipient_addr = Address::from_str(recipient)
        .map_err(|e| eyre::eyre!("Invalid recipient address: {}", e))?;

    // Create a wallet signer from the provided private key
    let wallet = PrivateKeySigner::from_slice(signer_privkey)
        .map_err(|e| eyre::eyre!("Invalid private key: {}", e))?;

    // Get the sender's address from the wallet
    let sender_addr = wallet.address();

    // Build a transaction request with standard ETH transfer parameters
    let tx_request = TransactionRequest::default()
        .with_to(recipient_addr)
        .with_value(U256::from(gwei_amount))
        .with_gas_limit(21_000) // Standard ETH transfer gas limit
        .with_max_priority_fee_per_gas(1_000_000_000) // 1 Gwei
        .with_max_fee_per_gas(20_000_000_000) // 20 Gwei
        .with_nonce(nonce)
        .with_chain_id(chain_id);

    // Convert the LocalSigner to an EthereumWallet to satisfy the NetworkWallet trait bound
    let ethereum_wallet = EthereumWallet::from(wallet);

    // Build and sign the transaction
    let tx_envelope = tx_request.build(&ethereum_wallet).await?;

    // Encode to raw bytes
    let raw_tx = tx_envelope.encoded_2718();

    Ok(Bytes::from(raw_tx))
}
