//! Example of signing, encoding and sending a raw transaction using a wallet.
//!
//! This module provides functionality to create and sign Ethereum transactions
//! for transferring ETH between addresses. It handles transaction building,
//! signing with private keys, and preparing transactions for network broadcast.

use alloy::{
    network::{Ethereum, Network, TransactionBuilder},
    primitives::{Address, U256},
    rpc::types::TransactionRequest,
};
use alloy_primitives::ChainId;
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
/// * `signer_privkey` - The private key of the sender (hex string)
/// * `recipient` - The recipient's Ethereum address (hex string)
/// * `chain_id` - The chain ID of the target network
/// * `gwei_amount` - The amount to transfer in wei
/// * `nonce` - The transaction nonce for the sender
///
/// # Returns
///
/// Returns a `Result` containing the signed transaction envelope if successful,
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
/// use ef_tests::create_transfer_transaction;
///
/// #[tokio::main]
/// async fn main() -> eyre::Result<()> {
///     let tx = create_transfer_transaction(
///         "0x123...", // private key
///         "0x456...", // recipient address
///         1,          // chain ID (mainnet)
///         1_000_000_000_000_000_000, // 1 ETH in wei
///         0           // nonce
///     ).await?;
///     
///     // tx can now be broadcast to the network
///     Ok(())
/// }
/// ```
pub async fn create_transfer_transaction(
    signer_privkey: &str,
    recipient: &str,
    chain_id: ChainId,
    gwei_amount: u64,
    nonce: u64,
) -> Result<<Ethereum as Network>::TxEnvelope> {
    // Parse the recipient address from string to Address type
    let recipient_addr = Address::from_str(recipient)
        .map_err(|e| eyre::eyre!("Invalid recipient address: {}", e))?;

    // Create a wallet signer from the provided private key
    let wallet = PrivateKeySigner::from_str(signer_privkey)
        .map_err(|e| eyre::eyre!("Invalid private key: {}", e))?;

    // Get the sender's address from the wallet
    let sender_addr = wallet.address();

    // Build a transaction request with standard ETH transfer parameters
    let tx = TransactionRequest::default()
        .with_from(sender_addr)
        .with_to(recipient_addr)
        .with_nonce(nonce)
        .with_chain_id(chain_id)
        .with_value(U256::from(gwei_amount))
        .with_gas_limit(21_000) // Standard gas limit for ETH transfers
        .with_max_priority_fee_per_gas(1_000_000_000) // 1 Gwei
        .with_max_fee_per_gas(20_000_000_000); // 20 Gwei

    // Convert the LocalSigner to an EthereumWallet to satisfy the NetworkWallet trait bound
    let ethereum_wallet = alloy::network::EthereumWallet::from(wallet);

    // Build and sign the transaction using the ethereum wallet
    let tx_envelope = tx
        .build(&ethereum_wallet)
        .await
        .map_err(|e| eyre::eyre!("Failed to build transaction: {}", e))?;

    Ok(tx_envelope)
}
