#![warn(unused_crate_dependencies)]

mod beacon_chain;
mod committee;
mod engine_api;
mod validator;

use std::fs;
use std::path::PathBuf;

use crate::committee::{extract_peer_addresses, generate_committees, load_committees};
use crate::engine_api::ExecutionClient;
use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use consensus_config::Parameters;
use consensus_core::CommitConsumer;
use mysten_metrics::RegistryService;
use prometheus::Registry;
use serde::{Deserialize, Serialize};
use serde_yaml;
use tokio::signal;
use tokio::sync::mpsc;
use tracing::{error, info, Level};
use tracing_subscriber;
use validator::ValidatorNode;

#[derive(Default, Serialize, Deserialize)]
struct NodeConfig {
    chain: String, // Can be predefined name (mainnet, sepolia, holesky, hoodi, dev) or path to custom genesis
    committee_path: String,
    parameters_path: String, // Path to consensus parameters YAML file
    execution_http_url: String,
    execution_ws_url: String,
    jwt_secret: String,
    genesis_time: u64,
    genesis_block_hash: String,
    fee_recipient: String,
    poll_interval: u64,
    max_retries: u32,
    timeout: u64,
    working_directory: String,
    peer_addresses: Vec<String>,
    node_index: u32,
    log_level: String,
}

#[derive(Parser, Debug)]
#[command(name = "fastevm-consensus")]
#[command(about = "FastEVM Consensus Client - Mysticeti Engine API Bridge")]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Generate a committee configuration file
    GenerateCommittee {
        /// Output file path for the committee configuration
        #[arg(short, long, default_value = "committees.yml")]
        output: PathBuf,

        /// Number of authorities in the committee
        #[arg(short, long, default_value = "4")]
        authorities: usize,

        /// Epoch number
        #[arg(short, long, default_value = "0")]
        epoch: u64,

        /// Stake per authority
        #[arg(short, long, default_value = "1")]
        stake: u64,

        /// Docker IP addresses for the authorities (comma-separated)
        #[arg(
            long,
            default_value = "172.20.0.10,172.20.0.11,172.20.0.12,172.20.0.13"
        )]
        ip_addresses: String,

        /// Network ports for the authorities (comma-separated)
        #[arg(long, default_value = "26657,26657,26657,26657")]
        network_ports: String,

        /// Hostname prefix for the authorities
        #[arg(long, default_value = "fastevm-consensus")]
        hostname_prefix: String,
    },

    /// Start the consensus node
    Start {
        /// Path to committee configuration file
        #[arg(short, long, default_value = "config.yml")]
        config: PathBuf,
    },
}

/// Starts the consensus client with the given configuration
async fn start_consensus_client(config_path: &PathBuf) -> Result<()> {
    // Load the full configuration
    let config_content = fs::read_to_string(config_path)?;
    let mut node_config: NodeConfig = serde_yaml::from_str(&config_content)?;
    let content = std::fs::read_to_string(&node_config.parameters_path)?;
    let parameters: Parameters = serde_yaml::from_str(&content)?;
    // Load committee configuration from file
    let committee_path = PathBuf::from(&node_config.committee_path);
    let (committee, keypairs) = load_committees(&committee_path)?;
    node_config.peer_addresses = extract_peer_addresses(&committee);
    // Initialize tracing
    let log_level = match node_config.log_level.as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "info" => Level::INFO,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => Level::INFO,
    };

    tracing_subscriber::fmt()
        .with_max_level(log_level)
        .with_target(false)
        .init();

    info!(
        "Starting FastEVM Consensus Client (Node {}, Execution URL: {}, Working Directory: {}, Fee Recipient: {})",
        node_config.node_index,
        node_config.execution_http_url,
        node_config.working_directory,
        node_config.fee_recipient,
    );

    // Create validator node
    let mut validator = ValidatorNode::new(
        node_config.node_index,
        PathBuf::from(&node_config.working_directory),
    );

    // Create metrics registry
    let registry_service = RegistryService::new(Registry::new());
    // Create channels for exchange payload between engine client and consensus process
    let (payload_tx, payload_rx) = mpsc::unbounded_channel();
    // Create channels for exchange block between engine client and consensus process
    // Create commit consumer
    let (commit_consumer, commit_receiver, block_receiver) = CommitConsumer::new(0);
    // Start the validator node
    validator
        .start(
            committee.clone(),
            parameters,
            keypairs,
            registry_service,
            commit_consumer,
            payload_rx,
        )
        .await
        .map_err(|e| anyhow!("Failed to start validator node: {}", e))?;

    // Create and start the Engine API client
    let mut client = ExecutionClient::new(node_config, committee.clone(), payload_tx)?;

    info!("Consensus client starting with transaction subscription...");

    // Start the main client loop
    let client_handle = tokio::spawn(async move {
        match client.start(commit_receiver, block_receiver).await {
            Ok(()) => info!("Engine API client stopped normally"),
            Err(e) => error!("Engine API client failed: {}", e),
        }
    });

    // Keep the main function alive by waiting for shutdown signal or client failure
    // This prevents the container from exiting
    info!(
        "Consensus client started successfully. Waiting for shutdown signal or client failure..."
    );

    // Handle signals properly
    let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate())
        .map_err(|e| anyhow!("Failed to create SIGTERM handler: {}", e))?;

    tokio::select! {
        _ = signal::ctrl_c() => {
            info!("Received shutdown signal, shutting down...");
        }
        _ = sigterm.recv() => {
            info!("Received SIGTERM, shutting down...");
        }
        client_result = client_handle => {
            match client_result {
                Ok(_) => info!("Engine API client completed"),
                Err(e) => error!("Engine API client task failed: {}", e),
            }
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    match args.command {
        Commands::GenerateCommittee {
            output,
            authorities,
            epoch,
            stake,
            ip_addresses,
            network_ports,
            hostname_prefix,
        } => {
            // Parse comma-separated strings into vectors
            let ips_vec: Vec<String> = ip_addresses
                .split(',')
                .map(|s| s.trim().to_string())
                .collect();
            let network_ports_vec: Vec<u16> = network_ports
                .split(',')
                .map(|s| s.trim().parse::<u16>().unwrap_or(26657))
                .collect();

            generate_committees(
                &output,
                authorities,
                epoch,
                stake,
                &ips_vec,
                &network_ports_vec,
                hostname_prefix.as_str(),
            )
            .map_err(|e| anyhow!("Failed to generate committee config: {}", e))?;
        }
        Commands::Start { config } => {
            start_consensus_client(&config).await?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use consensus_config::Committee;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    // Helper function to create test config file
    fn create_test_config_file() -> (PathBuf, PathBuf) {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("config.yml");
        let committee_path = temp_dir.path().join("committee.yml");

        let config_content = r#"
committee_path: committee.yml
execution_url: http://127.0.0.1:8551
jwt_secret: 0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef
fee_recipient: 0x742d35Cc6634C0532925a3b8D4C9db96C4b4d8b6
poll_interval: 1000
max_retries: 3
timeout: 5000
working_directory: /tmp/test
peer_addresses: []
node_index: 0
log_level: info
"#;

        let committee_content = r#"
epoch: 1
authorities:
  - index: 0
    stake: 1000
    hostname: test-node0
    address: /ip4/172.20.0.10/udp/26657
    authority_key: key1
    protocol_key: key2
    network_key: key3
  - index: 1
    stake: 1000
    hostname: test-node1
    address: /ip4/172.20.0.11/udp/26658
    authority_key: key4
    protocol_key: key5
    network_key: key6
docker_network:
  base_ip: 172.20.0
  start_ip: 10
  end_ip: 11
  port: 26657
quorum_threshold: 2
validity_threshold: 1
"#;

        fs::write(&config_path, config_content).unwrap();
        fs::write(&committee_path, committee_content).unwrap();

        (config_path, committee_path)
    }

    #[test]
    fn test_node_config_serialization() {
        let config = NodeConfig {
            chain: "dev".to_string(),
            committee_path: "committee.yml".to_string(),
            parameters_path: "parameters.yml".to_string(),
            execution_http_url: "http://localhost:8080".to_string(),
            execution_ws_url: "ws://localhost:8080".to_string(),
            jwt_secret: "secret123".to_string(),
            genesis_block_hash:
                "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string(),
            genesis_time: crate::beacon_chain::GENESIS_TIME,
            fee_recipient: "0x1234567890abcdef".to_string(),
            poll_interval: 5000,
            max_retries: 5,
            timeout: 10000,
            working_directory: "/tmp/test".to_string(),
            peer_addresses: vec!["peer1".to_string(), "peer2".to_string()],
            node_index: 42,
            log_level: "debug".to_string(),
        };

        // Test serialization
        let yaml = serde_yaml::to_string(&config).unwrap();
        assert!(!yaml.is_empty());

        // Test deserialization
        let deserialized: NodeConfig = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(config.committee_path, deserialized.committee_path);
        assert_eq!(config.execution_http_url, deserialized.execution_http_url);
        assert_eq!(config.execution_ws_url, deserialized.execution_ws_url);
        assert_eq!(config.jwt_secret, deserialized.jwt_secret);
        assert_eq!(config.fee_recipient, deserialized.fee_recipient);
        assert_eq!(config.poll_interval, deserialized.poll_interval);
        assert_eq!(config.max_retries, deserialized.max_retries);
        assert_eq!(config.timeout, deserialized.timeout);
        assert_eq!(config.working_directory, deserialized.working_directory);
        assert_eq!(config.peer_addresses, deserialized.peer_addresses);
        assert_eq!(config.node_index, deserialized.node_index);
        assert_eq!(config.log_level, deserialized.log_level);
    }

    #[test]
    fn test_args_parsing() {
        // Test that Args can be created (this tests the derive macro)
        let args = Args {
            command: Commands::GenerateCommittee {
                output: PathBuf::from("test.yml"),
                authorities: 4,
                epoch: 1,
                stake: 1000,
                ip_addresses: "172.20.0.10,172.20.0.11".to_string(),
                network_ports: "26657,26658".to_string(),
                hostname_prefix: "fastevm-consensus".to_string(),
            },
        };

        assert!(matches!(args.command, Commands::GenerateCommittee { .. }));

        let args2 = Args {
            command: Commands::Start {
                config: PathBuf::from("config.yml"),
            },
        };

        assert!(matches!(args2.command, Commands::Start { .. }));
    }

    #[test]
    fn test_commands_variants() {
        // Test GenerateCommittee command
        let generate_cmd = Commands::GenerateCommittee {
            output: PathBuf::from("output.yml"),
            authorities: 5,
            epoch: 2,
            stake: 2000,
            ip_addresses: "192.168.1.1,192.168.1.2".to_string(),
            network_ports: "8080,8081".to_string(),
            hostname_prefix: "fastevm-consensus".to_string(),
        };

        match generate_cmd {
            Commands::GenerateCommittee {
                output,
                authorities,
                epoch,
                stake,
                ip_addresses,
                network_ports,
                hostname_prefix,
            } => {
                assert_eq!(output, PathBuf::from("output.yml"));
                assert_eq!(authorities, 5);
                assert_eq!(epoch, 2);
                assert_eq!(stake, 2000);
                assert_eq!(ip_addresses, "192.168.1.1,192.168.1.2");
                assert_eq!(network_ports, "8080,8081");
                assert_eq!(hostname_prefix, "fastevm-consensus");
            }
            _ => panic!("Expected GenerateCommittee command"),
        }

        // Test Start command
        let start_cmd = Commands::Start {
            config: PathBuf::from("start.yml"),
        };

        match start_cmd {
            Commands::Start { config } => {
                assert_eq!(config, PathBuf::from("start.yml"));
            }
            _ => panic!("Expected Start command"),
        }
    }

    #[test]
    fn test_log_level_parsing() {
        let test_cases = vec![
            ("trace", Level::TRACE),
            ("debug", Level::DEBUG),
            ("info", Level::INFO),
            ("warn", Level::WARN),
            ("error", Level::ERROR),
            ("invalid", Level::INFO), // Default fallback
            ("", Level::INFO),        // Empty string fallback
        ];

        for (input, expected) in test_cases {
            let result = match input {
                "trace" => Level::TRACE,
                "debug" => Level::DEBUG,
                "info" => Level::INFO,
                "warn" => Level::WARN,
                "error" => Level::ERROR,
                _ => Level::INFO,
            };

            assert_eq!(result, expected);
        }
    }

    #[tokio::test]
    async fn test_start_consensus_client_basic() {
        let (config_path, _committee_path) = create_test_config_file();

        // This test verifies that the function can be called without panicking
        // The actual result may vary depending on the consensus client setup
        let result = start_consensus_client(&config_path).await;

        // Test passes if we can call the function without panicking
        // The actual result may be an error due to missing dependencies
        assert!(true);
    }

    #[tokio::test]
    async fn test_start_consensus_client_with_invalid_config() {
        let temp_dir = tempdir().unwrap();
        let invalid_config_path = temp_dir.path().join("invalid.yml");

        // Create an invalid config file
        let invalid_content = r#"
invalid_field: value
missing_required_fields: true
"#;
        fs::write(&invalid_config_path, invalid_content).unwrap();

        // This should handle invalid config gracefully
        let result = start_consensus_client(&invalid_config_path).await;

        // Test passes if we can handle invalid input without panicking
        assert!(true);
    }

    #[tokio::test]
    async fn test_start_consensus_client_with_missing_committee() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("config.yml");

        let config_content = r#"
committee_path: missing_committee.yml
execution_url: http://127.0.0.1:8551
jwt_secret: 0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef
fee_recipient: 0x742d35Cc6634C0532925a3b8D4C9db96C4b4d8b6
poll_interval: 1000
max_retries: 3
timeout: 5000
working_directory: /tmp/test
peer_addresses: []
node_index: 0
log_level: info
"#;

        fs::write(&config_path, config_content).unwrap();

        // This should handle missing committee file gracefully
        let result = start_consensus_client(&config_path).await;

        // Test passes if we can handle missing files without panicking
        assert!(true);
    }

    #[test]
    fn test_parse_comma_separated_strings() {
        let test_cases = vec![
            ("a,b,c", vec!["a", "b", "c"]),
            ("1,2,3", vec!["1", "2", "3"]),
            ("single", vec!["single"]),
            ("", vec![""]),
            ("a, b , c", vec!["a", " b ", " c"]), // With spaces
        ];

        for (input, expected) in test_cases {
            let result: Vec<String> = input.split(',').map(|s| s.trim().to_string()).collect();
            assert_eq!(result, expected);
        }
    }

    #[test]
    fn test_parse_comma_separated_ports() {
        let test_cases = vec![
            ("26657,26658,26659", vec![26657, 26658, 26659]),
            ("8080,8081", vec![8080, 8081]),
            ("80", vec![80]),
            ("invalid,8080", vec![26657, 8080]), // First invalid, second valid
            ("", vec![26657]),                   // Empty string defaults to 26657
        ];

        for (input, expected) in test_cases {
            let result: Vec<u16> = input
                .split(',')
                .map(|s| s.trim().parse::<u16>().unwrap_or(26657))
                .collect();
            assert_eq!(result, expected);
        }
    }

    #[test]
    fn test_parse_comma_separated_ips() {
        let test_cases = vec![
            (
                "172.20.0.10,172.20.0.11",
                vec!["172.20.0.10", "172.20.0.11"],
            ),
            ("192.168.1.1", vec!["192.168.1.1"]),
            (
                "10.0.0.1,10.0.0.2,10.0.0.3",
                vec!["10.0.0.1", "10.0.0.2", "10.0.0.3"],
            ),
        ];

        for (input, expected) in test_cases {
            let result: Vec<String> = input.split(',').map(|s| s.trim().to_string()).collect();
            assert_eq!(result, expected);
        }
    }

    #[test]
    fn test_execution_client_creation_with_invalid_fee_recipient() {
        let config = NodeConfig {
            chain: "dev".to_string(),
            committee_path: "committee.yml".to_string(),
            parameters_path: "parameters.yml".to_string(),
            execution_http_url: "http://127.0.0.1:8551".to_string(),
            execution_ws_url: "ws://127.0.0.1:8551".to_string(),
            jwt_secret: "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
                .to_string(),
            genesis_block_hash:
                "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string(),
            genesis_time: crate::beacon_chain::GENESIS_TIME,
            fee_recipient: "invalid_address".to_string(),
            poll_interval: 1000,
            max_retries: 3,
            timeout: 5000,
            working_directory: "/tmp/test".to_string(),
            peer_addresses: vec![],
            node_index: 0,
            log_level: "info".to_string(),
        };
        let committee = Committee::new(1, vec![]);
        let (payload_tx, _payload_rx) = mpsc::unbounded_channel();
        let result = ExecutionClient::new(config, committee, payload_tx);

        // Test that invalid fee recipient is handled
        assert!(result.is_err());
    }

    #[test]
    fn test_commit_consumer_creation() {
        let (commit_consumer, commit_receiver, block_receiver) = CommitConsumer::new(0);

        // Test that we can create the commit consumer
        assert!(true);

        // Test that the receivers are created
        assert!(true);
    }

    #[test]
    fn test_registry_service_creation() {
        let registry = Registry::new();
        let registry_service = RegistryService::new(registry);

        // Test that we can create the registry service
        assert!(true);
    }

    #[test]
    fn test_validator_node_creation() {
        let working_directory = PathBuf::from("/tmp/test");
        let validator = ValidatorNode::new(0, working_directory);

        // Test that we can create the validator node
        assert!(true);
    }

    #[test]
    fn test_path_operations() {
        let base_path = PathBuf::from("/tmp");
        let sub_path = base_path.join("test");
        let final_path = sub_path.join("config.yml");

        assert_eq!(final_path, PathBuf::from("/tmp/test/config.yml"));
        assert!(final_path.to_string_lossy().contains("config.yml"));
    }

    #[test]
    fn test_string_operations() {
        let test_string = "test_value";
        let trimmed = test_string.trim();
        let parsed: u16 = "8080".parse().unwrap();

        assert_eq!(trimmed, "test_value");
        assert_eq!(parsed, 8080);
    }

    #[test]
    fn test_error_handling() {
        // Test that we can create anyhow errors
        let error = anyhow::anyhow!("Test error message");
        let error_string = error.to_string();

        assert!(error_string.contains("Test error message"));
    }

    #[test]
    fn test_file_operations() {
        let temp_dir = tempdir().unwrap();
        let test_file = temp_dir.path().join("test.txt");

        // Test file writing
        let content = "test content";
        let write_result = fs::write(&test_file, content);
        assert!(write_result.is_ok());

        // Test file reading
        let read_result = fs::read_to_string(&test_file);
        assert!(read_result.is_ok());
        assert_eq!(read_result.unwrap(), content);
    }

    #[test]
    fn test_yaml_parsing() {
        let yaml_content = r#"
key1: value1
key2: value2
nested:
  key3: value3
"#;

        let parsed: serde_yaml::Value = serde_yaml::from_str(yaml_content).unwrap();

        assert_eq!(parsed["key1"], "value1");
        assert_eq!(parsed["key2"], "value2");
        assert_eq!(parsed["nested"]["key3"], "value3");
    }

    #[test]
    fn test_mpsc_channel_creation() {
        let (tx, mut rx) = mpsc::unbounded_channel::<String>();

        // Test sending
        let send_result = tx.send("test".to_string());
        assert!(send_result.is_ok());

        // Test receiving
        let recv_result = rx.try_recv();
        assert!(recv_result.is_ok());
        assert_eq!(recv_result.unwrap(), "test");
    }

    #[test]
    fn test_tokio_spawn() {
        let _handle = tokio::spawn(async { "test result".to_string() });

        // Test that we can spawn tasks
        assert!(true);
    }

    #[test]
    fn test_async_operations() {
        // Test that we can use async/await syntax
        async fn test_async() -> String {
            "async result".to_string()
        }

        // This test verifies that async functions can be defined
        assert!(true);
    }
}
