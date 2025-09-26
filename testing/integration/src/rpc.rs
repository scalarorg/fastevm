//! RPC utilities for integration testing
//!
//! This module provides utility functions for interacting with Ethereum RPC endpoints
//! for testing purposes. It includes functions to get transaction nonces from multiple
//! RPC endpoints and handle connection errors.

use alloy_primitives::Address;
use alloy_provider::{Provider, ProviderBuilder};
use std::collections::BTreeMap;

/// Gets the transaction nonces for the given addresses from the specified RPC URLs.
pub async fn get_nonces(addresses: &[Address], url: &str) -> BTreeMap<Address, u64> {
    let mut nonces = BTreeMap::new();
    // Get the initial nonce for the sender address
    let provider = match ProviderBuilder::new().connect(url).await {
        Ok(provider) => provider,
        Err(e) => {
            println!(
                "⚠️  Warning: Could not connect to Ethereum network at {:?}",
                &url
            );
            println!("   Error: {:?}", &e);
            println!("   Skipping test...");
            return nonces;
        }
    };
    for address in addresses.iter() {
        let address_nonce = match provider.get_transaction_count(*address).await {
            Ok(nonce) => nonce,
            Err(e) => {
                println!(
                    "⚠️  Warning: Could not get nonce for address {:?}",
                    &address
                );
                println!("   Error: {:?}", &e);
                println!("   Skipping test...");
                return nonces;
            }
        };
        nonces.insert(*address, address_nonce);
    }
    nonces
}
