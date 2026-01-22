//!
//! FastEVM CLI utilities
//!
//! Run with
//!
//! ```sh
//! cargo run -p execution-client --bin fastevm-cli -- show-peer-id --file /path/to/secret.key
//! cargo run -p execution-client --bin fastevm-cli -- show-peer-id --file /path/to/secret.key --output /path/to/output.txt
//! ```

use alloy_primitives::{Bytes, Uint};
use alloy_provider::ProviderBuilder;
use alloy_sol_macro::sol;
// use bip39::Mnemonic;
use clap::{Parser, Subcommand};
use greth::reth_pipe_exec_layer_ext_v2::onchain_config::BLOCK_ADDR;
use greth::reth_pipe_exec_layer_ext_v2::onchain_config::TIMESTAMP_ADDR;
use greth::reth_pipe_exec_layer_ext_v2::onchain_config::VALIDATOR_MANAGER_ADDR;
use reth_network_peers::pk2id;
use secp256k1::SecretKey;
use std::env;
use std::fs;
use std::path::PathBuf;

const DEFAULT_MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

sol! {
    #[sol(rpc)]
    #[derive(Debug)]
    enum ValidatorStatus {
        PENDING_ACTIVE, // 0
        ACTIVE, // 1
        PENDING_INACTIVE, // 2
        INACTIVE // 3
    }

    // Commission structure
    #[sol(rpc)]
    struct Commission {
        uint64 rate; // the commission rate charged to delegators(10000 is 100%)
        uint64 maxRate; // maximum commission rate which validator can ever charge
        uint64 maxChangeRate; // maximum daily increase of the validator commission
    }

    /// Complete validator information (merged from multiple contracts)
    /// #[sol(rpc)]
    struct ValidatorInfo {
        // Basic information (from ValidatorManager)
        bytes consensusPublicKey;
        Commission commission;
        string moniker;
        bool registered;
        address stakeCreditAddress;
        ValidatorStatus status;
        uint256 votingPower; // Changed from uint64 to uint256 to prevent overflow
        uint256 validatorIndex;
        uint256 updateTime;
        address operator;
        bytes validatorNetworkAddresses; // BCS serialized Vec<NetworkAddress>
        bytes fullnodeNetworkAddresses; // BCS serialized Vec<NetworkAddress>
        bytes aptosAddress; // [u8; 32]
    }
    #[sol(rpc)]
    struct ValidatorSet {
        ValidatorInfo[] activeValidators; // Active validators for the current epoch
        ValidatorInfo[] pendingInactive; // Pending validators to leave in next epoch (still active)
        ValidatorInfo[] pendingActive; // Pending validators to join in next epoch
        uint256 totalVotingPower; // Current total voting power
        uint256 totalJoiningPower; // Total voting power waiting to join in the next epoch
    }

    #[sol(rpc)]
    interface ValidatorManager {
        function getValidatorSet() external view returns (ValidatorSet memory);
        function getValidatorByProposer(bytes calldata proposer) external view returns (address, uint256);
    }
    #[sol(rpc)]
    interface Timestamp {
        function nowMicroseconds() external view returns (uint64);

    }
}

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
    GetValidatorSet,
    GetValidatorByProposer {
        #[arg(short, long)]
        proposer: String,
    },
    GetTimestamp,
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

pub async fn get_validator_set() -> eyre::Result<()> {
    // RPC URL
    let rpc_url = env::var("RPC_URL").unwrap_or_else(|_| "http://localhost:8545".to_string());

    // Provider
    let provider = ProviderBuilder::new().on_http(rpc_url.parse()?);

    // ✅ Correct: create contract instance
    let contract = ValidatorManager::new(VALIDATOR_MANAGER_ADDR, provider);

    // ✅ Correct: call returns ValidatorSet directly
    let validator_set = contract.getValidatorSet().call().await?;

    println!("Validator Set:");
    println!("  Total Voting Power: {}", validator_set.totalVotingPower);
    println!("  Total Joining Power: {}", validator_set.totalJoiningPower);
    println!(
        "  Active Validators: {}",
        validator_set.activeValidators.len()
    );
    println!(
        "  Pending Inactive: {}",
        validator_set.pendingInactive.len()
    );
    println!("  Pending Active: {}", validator_set.pendingActive.len());

    for (idx, v) in validator_set.activeValidators.iter().enumerate() {
        println!("Active Validator {}:", idx);
        println!("  Moniker: {}", v.moniker);
        println!("  Operator: {:?}", v.operator);
        println!("  Aptos Address: {:?}", v.aptosAddress);
        println!(
            "  Validator Network Addresses: {:?}",
            v.validatorNetworkAddresses
        );
        println!(
            "  Fullnode Network Addresses: {:?}",
            v.fullnodeNetworkAddresses
        );
        println!("  Voting Power: {}", v.votingPower);
        println!("  Validator Index: {}", v.validatorIndex);
    }

    Ok(())
}

pub async fn get_validator_by_proposer(proposer: String) -> eyre::Result<()> {
    // RPC URL
    let rpc_url = env::var("RPC_URL").unwrap_or_else(|_| "http://localhost:8545".to_string());

    // Parse proposer hex → bytes
    let proposer_hex = proposer.strip_prefix("0x").unwrap_or(&proposer);

    let proposer_bytes = hex::decode(proposer_hex)
        .map_err(|e| eyre::eyre!("Failed to parse proposer hex string: {}", e))?;

    let proposer_bytes = Bytes::from(proposer_bytes);

    // Provider
    let provider = ProviderBuilder::new().on_http(rpc_url.parse()?);

    // Contract
    let contract = ValidatorManager::new(VALIDATOR_MANAGER_ADDR, provider);

    // ✅ Call returns ValidatorInfo directly
    let result = contract
        .getValidatorByProposer(proposer_bytes)
        .call()
        .await?;

    // Print result
    println!("Validator Info:");
    println!("  Operator: {:?}", result._0);
    println!("  Validator Index: {}", result._1);

    Ok(())
}

async fn get_timestamp() -> eyre::Result<()> {
    // RPC URL
    let rpc_url = env::var("RPC_URL").unwrap_or_else(|_| "http://localhost:8545".to_string());

    // Provider
    let provider = ProviderBuilder::new().on_http(rpc_url.parse()?);

    // Contract
    let contract = Timestamp::new(TIMESTAMP_ADDR, provider);

    // Call
    let result = contract.nowMicroseconds().call().await?;

    println!("Timestamp: {}", result);

    Ok(())
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::ShowPeerId { file, output } => {
            show_peer_id(file, output)?;
        }
        Commands::GetValidatorSet => {
            get_validator_set().await?;
        }
        Commands::GetValidatorByProposer { proposer } => {
            get_validator_by_proposer(proposer).await?;
        }
        Commands::GetTimestamp => {
            get_timestamp().await?;
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

    // #[test]
    // fn test_cli_allocate_funds_command() {
    //     // Test AllocateFunds command parsing
    //     let args = vec![
    //         "fastevm-cli",
    //         "allocate-funds",
    //         "--input",
    //         "/path/to/genesis.json",
    //         "--count",
    //         "5",
    //         "--mnemonic",
    //         "test mnemonic phrase",
    //         "--amount",
    //         "1000000000000000000",
    //         "--output",
    //         "/path/to/output",
    //     ];
    //     let cli = Cli::try_parse_from(args).unwrap();

    //     match cli.command {
    //         Commands::AllocateFunds {
    //             input,
    //             count,
    //             mnemonic,
    //             amount,
    //             output,
    //         } => {
    //             assert_eq!(input, PathBuf::from("/path/to/genesis.json"));
    //             assert_eq!(count, 5);
    //             assert_eq!(mnemonic, Some("test mnemonic phrase".to_string()));
    //             assert_eq!(amount, "1000000000000000000");
    //             assert_eq!(output, PathBuf::from("/path/to/output"));
    //         }
    //         _ => panic!("Expected AllocateFunds command"),
    //     }
    // }
}
