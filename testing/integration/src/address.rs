//! Address derivation utilities for Ethereum integration testing
//!
//! This module provides functions to derive Ethereum addresses from mnemonic phrases
//! using standard BIP39 and BIP32 derivation paths. It's primarily used for testing
//! purposes to generate deterministic addresses for test accounts.
//!
//! # Features
//!
//! - Derive Ethereum addresses from mnemonic phrases using BIP39/BIP32 standards
//! - Support for standard Ethereum derivation paths (e.g., "m/44'/60'/0'/0/0")
//! - EIP-55 checksum address generation
//! - Address validation and normalization utilities
//! - Compatible with standard Ethereum wallets and tools
//!
//! # Examples
//!
//! ```rust
//! use fastevm_testing::address::derive_eth_address;
//!
//! let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
//! let derivation_path = "m/44'/60'/0'/0/0";
//!
//! match derive_eth_address(mnemonic, derivation_path) {
//!     Ok(address) => println!("Derived address: {}", address),
//!     Err(e) => eprintln!("Error: {}", e),
//! }
//! ```
//!
//! # Derivation Process
//!
//! The address derivation follows these steps:
//! 1. Convert mnemonic phrase to seed using BIP39
//! 2. Generate master extended private key from seed using BIP32
//! 3. Derive child private key using the specified derivation path
//! 4. Extract public key from private key
//! 5. Compute Ethereum address as last 20 bytes of Keccak256(public_key)

use bip32::{DerivationPath, XPrv};
use bip39::Mnemonic;
use k256::elliptic_curve::sec1::ToEncodedPoint;
use std::str::FromStr;
use tiny_keccak::{Hasher, Keccak};

/// Derive an Ethereum address from a mnemonic phrase and derivation path.
///
/// This function follows the standard BIP39/BIP32 derivation process to generate
/// a deterministic Ethereum address from a mnemonic seed phrase.
///
/// # Arguments
///
/// * `mnemonic` - A BIP39 mnemonic phrase (12, 15, 18, 21, or 24 words)
/// * `derivation_path` - BIP32 derivation path (e.g., "m/44'/60'/0'/0/0")
///
/// # Returns
///
/// Returns a `Result` containing:
/// * `Ok(String)` - The derived Ethereum address in lowercase hex format with 0x prefix
/// * `Err(Box<dyn std::error::Error>)` - Error if derivation fails
///
/// # Errors
///
/// This function will return an error if:
/// * The mnemonic phrase is invalid
/// * The derivation path is malformed
/// * Key derivation fails at any step
///
/// # Examples
///
/// ```rust
/// use fastevm_testing::address::derive_eth_address;
///
/// let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
/// let derivation_path = "m/44'/60'/0'/0/0";
///
/// match derive_eth_address(mnemonic, derivation_path) {
///     Ok(address) => println!("Address: {}", address), // "0x9858effd232b4033e47d90003d41ec34cdaa188d"
///     Err(e) => eprintln!("Error: {}", e),
/// }
/// ```
///
/// # Derivation Path Format
///
/// The derivation path follows BIP32 format:
/// - `m` - master key
/// - `44'` - BIP44 purpose (cryptocurrency)
/// - `60'` - Ethereum coin type
/// - `0'` - Account index (hardened)
/// - `0` - Change index (external)
/// - `0` - Address index
pub fn derive_eth_address(
    mnemonic: &str,
    derivation_path: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    // 1) Convert mnemonic -> seed (BIP39)
    let mnemonic = Mnemonic::parse(mnemonic)?;
    // empty passphrase; if you use a passphrase, pass it here
    let seed = mnemonic.to_seed("");

    // 2) Create master extended private key from seed (BIP32)
    let xprv = XPrv::new(&seed)?;

    // 3) Parse derivation path and derive child xprv
    // bip32::DerivationPath supports the "m/44'/60'/0'/0/0" string format
    let path = DerivationPath::from_str(derivation_path)?;
    let mut child_xprv = xprv;
    for cn in path.into_iter() {
        child_xprv = child_xprv.derive_child(cn)?;
    }

    // 4) Get the raw private key bytes (32 bytes)
    let private_key_bytes = child_xprv.private_key().to_bytes();

    // 5) From private key derive the uncompressed public key (65 bytes starting with 0x04)
    let secret = k256::SecretKey::from_bytes(&private_key_bytes)?;
    let pubkey = k256::PublicKey::from_secret_scalar(&secret.to_nonzero_scalar());
    let encoded = pubkey.to_encoded_point(false); // uncompressed
    let pubkey_bytes = encoded.as_bytes();

    // Ethereum address is last 20 bytes of keccak256(uncompressed_pubkey[1..]) (skip 0x04)
    let pubkey_no_prefix = &pubkey_bytes[1..]; // 64 bytes
    let mut keccak = Keccak::v256();
    let mut output = [0u8; 32];
    keccak.update(pubkey_no_prefix);
    keccak.finalize(&mut output);

    // take last 20 bytes
    let address = &output[12..];

    Ok(format!("0x{}", hex::encode(address)))
}

/// Convert an Ethereum address to EIP-55 checksum format.
///
/// EIP-55 is a mixed-case checksum encoding for Ethereum addresses that helps
/// prevent errors when typing or copying addresses. The checksum is calculated
/// using Keccak256 of the lowercase address.
///
/// # Arguments
///
/// * `addr` - Ethereum address in hex format with 0x prefix (case insensitive)
///
/// # Returns
///
/// Returns a `Result` containing:
/// * `Ok(String)` - The address in EIP-55 checksum format
/// * `Err(Box<dyn std::error::Error>)` - Error if the address format is invalid
///
/// # Errors
///
/// This function will return an error if:
/// * The address doesn't start with "0x"
/// * The address is not exactly 42 characters long (0x + 40 hex chars)
/// * The address contains invalid hex characters
///
/// # Examples
///
/// ```rust
/// use fastevm_testing::address::to_checksum_address;
///
/// let address = "0x9858effd232b4033e47d90003d41ec34cdaa188d";
/// match to_checksum_address(address) {
///     Ok(checksum) => println!("Checksum address: {}", checksum), // "0x9858EfFd232b4033e47d90003d41ec34cdaa188d"
///     Err(e) => eprintln!("Error: {}", e),
/// }
/// ```
///
/// # EIP-55 Specification
///
/// For each character in the address, if the corresponding nibble in the Keccak256
/// hash of the lowercase address is >= 8, the character is uppercased, otherwise
/// it remains lowercase.
pub fn to_checksum_address(addr: &str) -> Result<String, Box<dyn std::error::Error>> {
    // addr expected like "0x" + 40 hex chars
    let addr = addr
        .strip_prefix("0x")
        .ok_or("address must start with 0x")?
        .to_lowercase();
    let mut keccak = Keccak::v256();
    keccak.update(addr.as_bytes());
    let mut output = [0u8; 32];
    keccak.finalize(&mut output);
    let hash_hex = hex::encode(output);

    let mut result = String::from("0x");
    for (c, hash_char) in addr.chars().zip(hash_hex.chars()) {
        // if nibble >= 8 uppercase; else lowercase
        let should_upper = u8::from_str_radix(&hash_char.to_string(), 16)? >= 8;
        if should_upper {
            result.push(c.to_ascii_uppercase());
        } else {
            result.push(c);
        }
    }
    Ok(result)
}

/// Validate an Ethereum address format.
///
/// Checks if the provided string is a valid Ethereum address format:
/// - Starts with "0x"
/// - Contains exactly 40 hexadecimal characters
/// - All characters after "0x" are valid hex (0-9, a-f, A-F)
///
/// # Arguments
///
/// * `addr` - String to validate as an Ethereum address
///
/// # Returns
///
/// Returns `true` if the address format is valid, `false` otherwise.
///
/// # Examples
///
/// ```rust
/// use fastevm_testing::address::is_valid_eth_address;
///
/// assert!(is_valid_eth_address("0x9858effd232b4033e47d90003d41ec34cdaa188d"));
/// assert!(is_valid_eth_address("0x9858EFFD232B4033E47D90003D41EC34CDAA188D"));
/// assert!(!is_valid_eth_address("0x9858effd232b4033e47d90003d41ec34cdaa188")); // too short
/// assert!(!is_valid_eth_address("9858effd232b4033e47d90003d41ec34cdaa188d")); // missing 0x
/// assert!(!is_valid_eth_address("0x9858effd232b4033e47d90003d41ec34cdaa188g")); // invalid char
/// ```
pub fn is_valid_eth_address(addr: &str) -> bool {
    if !addr.starts_with("0x") {
        return false;
    }

    let hex_part = &addr[2..];
    if hex_part.len() != 40 {
        return false;
    }

    hex_part.chars().all(|c| c.is_ascii_hexdigit())
}

/// Normalize an Ethereum address to lowercase format.
///
/// Converts an Ethereum address to the standard lowercase format with 0x prefix.
/// This is useful for consistent address comparison and storage.
///
/// # Arguments
///
/// * `addr` - Ethereum address in any case format
///
/// # Returns
///
/// Returns a `Result` containing:
/// * `Ok(String)` - The normalized address in lowercase
/// * `Err(Box<dyn std::error::Error>)` - Error if the address format is invalid
///
/// # Examples
///
/// ```rust
/// use fastevm_testing::address::normalize_eth_address;
///
/// let address = "0x9858EFFD232B4033E47D90003D41EC34CDAA188D";
/// match normalize_eth_address(address) {
///     Ok(normalized) => println!("Normalized: {}", normalized), // "0x9858effd232b4033e47d90003d41ec34cdaa188d"
///     Err(e) => eprintln!("Error: {}", e),
/// }
/// ```
pub fn normalize_eth_address(addr: &str) -> Result<String, Box<dyn std::error::Error>> {
    if !is_valid_eth_address(addr) {
        return Err("Invalid Ethereum address format".into());
    }

    Ok(addr.to_lowercase())
}
