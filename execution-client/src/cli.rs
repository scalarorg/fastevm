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
