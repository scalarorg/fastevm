//!
//! FastEVM CLI utilities
//!
//! Run with
//!
//! ```sh
//! cargo run -p execution-client --bin fastevm-cli -- show-peer-id --file /path/to/secret.key
//! cargo run -p execution-client --bin fastevm-cli -- show-peer-id --file /path/to/secret.key --output /path/to/output.txt
//! ```

use clap::{Parser, Subcommand};
use reth_network_peers::pk2id;
use secp256k1::SecretKey;
use std::fs;
use std::path::PathBuf;

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

fn main() -> eyre::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::ShowPeerId { file, output } => {
            show_peer_id(file, output)?;
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
        }
    }
}
