//!
//! FastEVM CLI utilities
//!
//! Run with
//!
//! ```sh
//! cargo run -p execution-client --bin fastevm-cli -- show-peer-id --file /path/to/secret.key
//! cargo run -p execution-client --bin fastevm-cli -- show-peer-id --file /path/to/secret.key --output /path/to/output.txt
//! ```

use alloy_primitives::{Address, U256};
use bip39::Mnemonic;
use clap::{Parser, Subcommand};
use reth_network_peers::pk2id;
use secp256k1::SecretKey;
use serde_json::{Map, Value};
use std::fs;
use std::path::PathBuf;

const DEFAULT_MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

#[derive(Parser)]
#[command(name = "fastevm-cli")]
#[command(about = "FastEVM CLI utilities")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show peer ID from secret key file
    ShowPeerId {
        /// Path to the secret key file
        #[arg(short, long)]
        file: PathBuf,
        /// Output file to write peer ID (optional)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Allocate funds to generated accounts in genesis.json
    AllocateFunds {
        /// Path to input genesis.json file
        #[arg(short, long)]
        input: PathBuf,
        /// Number of accounts to generate
        #[arg(short, long, default_value = "100")]
        count: u32,
        /// Mnemonic phrase for account generation (optional, uses default if not provided)
        #[arg(short, long)]
        mnemonic: Option<String>,
        /// Amount of funds to allocate to each account (in wei, default: 1000 ETH)
        #[arg(short, long, default_value = "1000000000000000000000")]
        amount: String,
        /// Output directory to save modified genesis.json
        #[arg(short, long)]
        output: PathBuf,
    },
}

fn show_peer_id(file_path: PathBuf, output_path: Option<PathBuf>) -> eyre::Result<()> {
    // Read the secret key from file
    let secret_key_bytes = fs::read_to_string(&file_path)
        .map_err(|e| eyre::eyre!("Failed to read file {}: {}", file_path.display(), e))?
        .trim()
        .to_string();

    // Remove "0x" prefix if present
    let secret_key_hex = if secret_key_bytes.starts_with("0x") {
        &secret_key_bytes[2..]
    } else {
        &secret_key_bytes
    };

    // Parse the hex string to bytes
    let secret_key_bytes = hex::decode(secret_key_hex)
        .map_err(|e| eyre::eyre!("Failed to parse hex string: {}", e))?;

    // Create a SecretKey from the bytes
    let secret_key = SecretKey::from_slice(&secret_key_bytes)
        .map_err(|e| eyre::eyre!("Failed to create secret key: {}", e))?;

    // Convert to peer ID using pk2id (which expects a PublicKey)
    let peer_id = pk2id(&secret_key.public_key(&secp256k1::Secp256k1::new()));

    println!("Secret key file: {}", file_path.display());
    println!("Peer ID: {}", peer_id);

    // Write peer ID to output file if specified
    if let Some(output_path) = output_path {
        fs::write(&output_path, peer_id.to_string()).map_err(|e| {
            eyre::eyre!(
                "Failed to write peer ID to {}: {}",
                output_path.display(),
                e
            )
        })?;
        println!("Peer ID written to: {}", output_path.display());
    }

    Ok(())
}

fn allocate_funds(
    input_path: PathBuf,
    count: u32,
    mnemonic: Option<String>,
    amount: String,
    output_path: PathBuf,
) -> eyre::Result<()> {
    // Parse the amount string to U256
    let amount_wei = if amount.starts_with("0x") {
        U256::from_str_radix(&amount[2..], 16)
            .map_err(|e| eyre::eyre!("Invalid hex amount: {}", e))?
    } else {
        U256::from_str_radix(&amount, 10)
            .map_err(|e| eyre::eyre!("Invalid decimal amount: {}", e))?
    };

    // Use default mnemonic if not provided
    let mnemonic_phrase = mnemonic.unwrap_or_else(|| DEFAULT_MNEMONIC.to_string());

    // Parse the mnemonic
    let mnemonic =
        Mnemonic::parse(&mnemonic_phrase).map_err(|e| eyre::eyre!("Invalid mnemonic: {}", e))?;

    // Generate seed from mnemonic
    let seed = mnemonic.to_seed("");
    let seed_bytes = &seed[..];

    // Read and parse the input genesis.json
    let genesis_content = fs::read_to_string(&input_path).map_err(|e| {
        eyre::eyre!(
            "Failed to read genesis file {}: {}",
            input_path.display(),
            e
        )
    })?;

    let mut genesis: Value = serde_json::from_str(&genesis_content)
        .map_err(|e| eyre::eyre!("Failed to parse genesis.json: {}", e))?;

    // Get the alloc section or create it if it doesn't exist
    let alloc = genesis
        .get_mut("alloc")
        .ok_or_else(|| eyre::eyre!("Genesis file missing 'alloc' section"))?
        .as_object_mut()
        .ok_or_else(|| eyre::eyre!("'alloc' section is not an object"))?;

    // Generate accounts and add them to the alloc section
    for i in 0..count {
        let account = generate_account_from_seed(&seed_bytes, i)?;
        let address_hex = format!("0x{:x}", account.address);
        // Create account entry with balance
        let mut account_entry = Map::new();
        account_entry.insert(
            "balance".to_string(),
            Value::String(format!("0x{:x}", amount_wei)),
        );

        alloc.insert(address_hex, Value::Object(account_entry));
    }

    // Create output directory if it doesn't exist
    fs::create_dir_all(&output_path).map_err(|e| {
        eyre::eyre!(
            "Failed to create output directory {}: {}",
            output_path.display(),
            e
        )
    })?;

    // Write the modified genesis.json to output directory
    let output_file = output_path.join("genesis.json");
    let genesis_json = serde_json::to_string_pretty(&genesis)
        .map_err(|e| eyre::eyre!("Failed to serialize genesis.json: {}", e))?;

    fs::write(&output_file, genesis_json).map_err(|e| {
        eyre::eyre!(
            "Failed to write genesis.json to {}: {}",
            output_file.display(),
            e
        )
    })?;

    println!("Successfully allocated funds to {} accounts", count);
    println!("Modified genesis.json saved to: {}", output_file.display());
    println!("Mnemonic used: {}", mnemonic_phrase);

    Ok(())
}

#[derive(Debug)]
struct Account {
    address: Address,
    private_key: [u8; 32],
}

fn generate_account_from_seed(seed: &[u8], index: u32) -> eyre::Result<Account> {
    use sha2::{Digest, Sha256};
    // Simple deterministic key generation using seed + index
    let mut hasher = Sha256::new();
    hasher.update(seed);
    hasher.update(&index.to_le_bytes());
    let hash = hasher.finalize();

    // Use the hash as private key (first 32 bytes)
    let mut private_key = [0u8; 32];
    private_key.copy_from_slice(&hash[..32]);

    // Create secp256k1 secret key
    let secp = secp256k1::Secp256k1::new();
    let secret_key = SecretKey::from_slice(&private_key)
        .map_err(|e| eyre::eyre!("Invalid private key: {}", e))?;

    // Get the public key and derive address
    let public_key = secret_key.public_key(&secp);
    let public_key_bytes = public_key.serialize_uncompressed();

    // Ethereum address is the last 20 bytes of keccak256 hash of the public key
    let hash = alloy_primitives::keccak256(&public_key_bytes[1..]);
    let mut address_bytes = [0u8; 20];
    address_bytes.copy_from_slice(&hash[12..]);

    let address = Address::from(address_bytes);

    Ok(Account {
        address,
        private_key,
    })
}

fn main() -> eyre::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::ShowPeerId { file, output } => {
            show_peer_id(file, output)?;
        }
        Commands::AllocateFunds {
            input,
            count,
            mnemonic,
            amount,
            output,
        } => {
            allocate_funds(input, count, mnemonic, amount, output)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_show_peer_id_with_valid_hex() {
        // Create a temporary file with a valid secret key
        let mut temp_file = NamedTempFile::new().unwrap();
        let secret_key_hex = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        temp_file.write_all(secret_key_hex.as_bytes()).unwrap();
        temp_file.flush().unwrap();

        let result = show_peer_id(temp_file.path().to_path_buf(), None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_show_peer_id_with_0x_prefix() {
        // Create a temporary file with a valid secret key with 0x prefix
        let mut temp_file = NamedTempFile::new().unwrap();
        let secret_key_hex = "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        temp_file.write_all(secret_key_hex.as_bytes()).unwrap();
        temp_file.flush().unwrap();

        let result = show_peer_id(temp_file.path().to_path_buf(), None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_show_peer_id_with_output_file() {
        // Create a temporary file with a valid secret key
        let mut temp_file = NamedTempFile::new().unwrap();
        let secret_key_hex = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        temp_file.write_all(secret_key_hex.as_bytes()).unwrap();
        temp_file.flush().unwrap();

        // Create output file
        let output_file = NamedTempFile::new().unwrap();
        let output_path = output_file.path().to_path_buf();

        let result = show_peer_id(temp_file.path().to_path_buf(), Some(output_path.clone()));
        assert!(result.is_ok());

        // Verify output file was created and contains peer ID
        let output_content = fs::read_to_string(&output_path).unwrap();
        assert!(!output_content.is_empty());
    }

    #[test]
    fn test_show_peer_id_invalid_hex() {
        // Create a temporary file with invalid hex
        let mut temp_file = NamedTempFile::new().unwrap();
        let invalid_hex = "invalid_hex_string";
        temp_file.write_all(invalid_hex.as_bytes()).unwrap();
        temp_file.flush().unwrap();

        let result = show_peer_id(temp_file.path().to_path_buf(), None);
        assert!(result.is_err());
    }

    #[test]
    fn test_show_peer_id_file_not_found() {
        let non_existent_path = PathBuf::from("/non/existent/path");
        let result = show_peer_id(non_existent_path, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_show_peer_id_invalid_secret_key_length() {
        // Create a temporary file with invalid secret key length
        let mut temp_file = NamedTempFile::new().unwrap();
        let invalid_hex = "0123456789abcdef"; // Too short
        temp_file.write_all(invalid_hex.as_bytes()).unwrap();
        temp_file.flush().unwrap();

        let result = show_peer_id(temp_file.path().to_path_buf(), None);
        assert!(result.is_err());
    }

    #[test]
    fn test_cli_commands_parsing() {
        // Test ShowPeerId command parsing
        let args = vec!["fastevm-cli", "show-peer-id", "--file", "/path/to/file"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            Commands::ShowPeerId { file, output } => {
                assert_eq!(file, PathBuf::from("/path/to/file"));
                assert!(output.is_none());
            }
            _ => panic!("Expected ShowPeerId command"),
        }
    }

    #[test]
    fn test_cli_commands_with_output() {
        // Test ShowPeerId command with output file
        let args = vec![
            "fastevm-cli",
            "show-peer-id",
            "--file",
            "/path/to/file",
            "--output",
            "/path/to/output",
        ];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            Commands::ShowPeerId { file, output } => {
                assert_eq!(file, PathBuf::from("/path/to/file"));
                assert_eq!(output, Some(PathBuf::from("/path/to/output")));
            }
            _ => panic!("Expected ShowPeerId command"),
        }
    }

    #[test]
    fn test_cli_allocate_funds_command() {
        // Test AllocateFunds command parsing
        let args = vec![
            "fastevm-cli",
            "allocate-funds",
            "--input",
            "/path/to/genesis.json",
            "--count",
            "5",
            "--mnemonic",
            "test mnemonic phrase",
            "--amount",
            "1000000000000000000",
            "--output",
            "/path/to/output",
        ];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            Commands::AllocateFunds {
                input,
                count,
                mnemonic,
                amount,
                output,
            } => {
                assert_eq!(input, PathBuf::from("/path/to/genesis.json"));
                assert_eq!(count, 5);
                assert_eq!(mnemonic, Some("test mnemonic phrase".to_string()));
                assert_eq!(amount, "1000000000000000000");
                assert_eq!(output, PathBuf::from("/path/to/output"));
            }
            _ => panic!("Expected AllocateFunds command"),
        }
    }
}
