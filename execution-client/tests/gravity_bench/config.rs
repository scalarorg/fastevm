//! Configuration module for gravity_bench integration
//!
//! This module provides configuration builders and types for setting up
//! benchmark runs with gravity_bench.

use std::path::PathBuf;

/// Node configuration for connecting to Ethereum-compatible nodes
#[derive(Debug, Clone)]
pub struct NodeConfig {
    /// RPC URL of the node
    pub rpc_url: String,
    /// Chain ID of the network
    pub chain_id: u64,
}

impl NodeConfig {
    /// Create a new node configuration
    pub fn new(rpc_url: impl Into<String>, chain_id: u64) -> Self {
        Self {
            rpc_url: rpc_url.into(),
            chain_id,
        }
    }

    /// Create a localhost node configuration
    pub fn localhost(chain_id: u64) -> Self {
        Self::new("http://localhost:8545", chain_id)
    }

    /// Create a localhost node configuration with custom port
    pub fn localhost_with_port(port: u16, chain_id: u64) -> Self {
        Self::new(format!("http://localhost:{}", port), chain_id)
    }
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self::localhost(7771625) // Default FastEVM chain ID
    }
}

/// Faucet configuration for funding test accounts
#[derive(Debug, Clone)]
pub struct FaucetConfig {
    /// Private key of the faucet account (hex encoded without 0x prefix)
    pub private_key: String,
    /// Faucet level for cascading distribution
    /// - Value 10: Enables cascade mode with progression 1 -> 10 -> 100
    /// - Value 0: Disables cascading, processes data directly
    pub faucet_level: u32,
    /// Wait duration in seconds between faucet distribution levels
    pub wait_duration_secs: u64,
}

impl FaucetConfig {
    /// Create a new faucet configuration
    pub fn new(private_key: impl Into<String>) -> Self {
        Self {
            private_key: private_key.into(),
            faucet_level: 10,
            wait_duration_secs: 60,
        }
    }

    /// Set the faucet level
    pub fn with_faucet_level(mut self, level: u32) -> Self {
        self.faucet_level = level;
        self
    }

    /// Set the wait duration between distribution levels
    pub fn with_wait_duration(mut self, secs: u64) -> Self {
        self.wait_duration_secs = secs;
        self
    }

    /// Create a test configuration with a known test private key
    /// WARNING: Only use for testing, never in production
    pub fn test_default() -> Self {
        // This is a well-known test private key, do not use in production
        Self::new("ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80")
    }
}

/// Account configuration for load testing
#[derive(Debug, Clone)]
pub struct AccountsConfig {
    /// Number of load testing accounts to generate
    pub num_accounts: u32,
}

impl AccountsConfig {
    /// Create a new accounts configuration
    pub fn new(num_accounts: u32) -> Self {
        Self { num_accounts }
    }
}

impl Default for AccountsConfig {
    fn default() -> Self {
        Self::new(1000)
    }
}

/// Performance configuration for stress testing
#[derive(Debug, Clone)]
pub struct PerformanceConfig {
    /// Number of concurrent transaction sending tasks
    pub num_senders: u32,
    /// Maximum capacity of the transaction pool
    pub max_pool_size: u32,
    /// Duration of the benchmark in seconds (0 for indefinite)
    pub duration_secs: u64,
}

impl PerformanceConfig {
    /// Create a new performance configuration
    pub fn new(num_senders: u32, duration_secs: u64) -> Self {
        Self {
            num_senders,
            max_pool_size: 100000,
            duration_secs,
        }
    }

    /// Set the maximum pool size
    pub fn with_max_pool_size(mut self, size: u32) -> Self {
        self.max_pool_size = size;
        self
    }
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self::new(100, 60)
    }
}

/// Main benchmark configuration
#[derive(Debug, Clone)]
pub struct BenchConfig {
    /// Contract configuration file path
    pub contract_config_path: PathBuf,
    /// Target transactions per second
    pub target_tps: u32,
    /// List of nodes to connect to
    pub nodes: Vec<NodeConfig>,
    /// Number of ERC20 tokens to deploy
    pub num_tokens: u32,
    /// Enable Uniswap V2 swap benchmarking
    pub enable_swap_token: bool,
    /// Faucet configuration
    pub faucet: FaucetConfig,
    /// Accounts configuration
    pub accounts: AccountsConfig,
    /// Performance configuration
    pub performance: PerformanceConfig,
    /// Enable recovery mode (skip setup, use existing contracts/accounts)
    pub recovery_mode: bool,
}

impl BenchConfig {
    /// Create a new benchmark configuration builder
    pub fn builder() -> BenchConfigBuilder {
        BenchConfigBuilder::default()
    }

    /// Generate TOML configuration string
    pub fn to_toml(&self) -> String {
        let nodes_str: Vec<String> = self
            .nodes
            .iter()
            .map(|n| {
                format!(
                    "{{ rpc_url = \"{}\", chain_id = {} }}",
                    n.rpc_url, n.chain_id
                )
            })
            .collect();

        format!(
            r#"# Gravity Bench Configuration (Generated)
contract_config_path = "{}"
target_tps = {}
nodes = [
    {}
]
num_tokens = {}
enable_swap_token = {}

[faucet]
private_key = "{}"
faucet_level = {}
wait_duration_secs = {}

[accounts]
num_accounts = {}

[performance]
num_senders = {}
max_pool_size = {}
duration_secs = {}
"#,
            self.contract_config_path.display(),
            self.target_tps,
            nodes_str.join(",\n    "),
            self.num_tokens,
            self.enable_swap_token,
            self.faucet.private_key,
            self.faucet.faucet_level,
            self.faucet.wait_duration_secs,
            self.accounts.num_accounts,
            self.performance.num_senders,
            self.performance.max_pool_size,
            self.performance.duration_secs,
        )
    }

    /// Write configuration to a file
    pub fn write_to_file(&self, path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
        std::fs::write(path, self.to_toml())
    }
}

/// Builder for BenchConfig
#[derive(Debug, Default)]
pub struct BenchConfigBuilder {
    contract_config_path: Option<PathBuf>,
    target_tps: Option<u32>,
    nodes: Vec<NodeConfig>,
    num_tokens: Option<u32>,
    enable_swap_token: Option<bool>,
    faucet: Option<FaucetConfig>,
    accounts: Option<AccountsConfig>,
    performance: Option<PerformanceConfig>,
    recovery_mode: bool,
}

impl BenchConfigBuilder {
    /// Set the contract configuration file path
    pub fn contract_config_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.contract_config_path = Some(path.into());
        self
    }

    /// Set the target TPS
    pub fn target_tps(mut self, tps: u32) -> Self {
        self.target_tps = Some(tps);
        self
    }

    /// Add a node to connect to
    pub fn add_node(mut self, node: NodeConfig) -> Self {
        self.nodes.push(node);
        self
    }

    /// Set the RPC URL (convenience method for single node)
    pub fn rpc_url(mut self, url: impl Into<String>) -> Self {
        if self.nodes.is_empty() {
            self.nodes.push(NodeConfig::new(url, 7771625));
        } else {
            self.nodes[0].rpc_url = url.into();
        }
        self
    }

    /// Set the chain ID (convenience method for single node)
    pub fn chain_id(mut self, chain_id: u64) -> Self {
        if self.nodes.is_empty() {
            self.nodes.push(NodeConfig::localhost(chain_id));
        } else {
            self.nodes[0].chain_id = chain_id;
        }
        self
    }

    /// Set the number of tokens
    pub fn num_tokens(mut self, num: u32) -> Self {
        self.num_tokens = Some(num);
        self
    }

    /// Enable or disable swap token benchmarking
    pub fn enable_swap_token(mut self, enable: bool) -> Self {
        self.enable_swap_token = Some(enable);
        self
    }

    /// Set the faucet configuration
    pub fn faucet(mut self, faucet: FaucetConfig) -> Self {
        self.faucet = Some(faucet);
        self
    }

    /// Set the faucet private key (convenience method)
    pub fn faucet_private_key(mut self, key: impl Into<String>) -> Self {
        self.faucet = Some(FaucetConfig::new(key));
        self
    }

    /// Set the accounts configuration
    pub fn accounts(mut self, accounts: AccountsConfig) -> Self {
        self.accounts = Some(accounts);
        self
    }

    /// Set the number of accounts (convenience method)
    pub fn num_accounts(mut self, num: u32) -> Self {
        self.accounts = Some(AccountsConfig::new(num));
        self
    }

    /// Set the performance configuration
    pub fn performance(mut self, performance: PerformanceConfig) -> Self {
        self.performance = Some(performance);
        self
    }

    /// Set the duration in seconds (convenience method)
    pub fn duration_secs(mut self, secs: u64) -> Self {
        let perf = self.performance.unwrap_or_default();
        self.performance = Some(PerformanceConfig {
            duration_secs: secs,
            ..perf
        });
        self
    }

    /// Set the number of senders (convenience method)
    pub fn num_senders(mut self, num: u32) -> Self {
        let perf = self.performance.unwrap_or_default();
        self.performance = Some(PerformanceConfig {
            num_senders: num,
            ..perf
        });
        self
    }

    /// Enable recovery mode
    pub fn recovery_mode(mut self, enable: bool) -> Self {
        self.recovery_mode = enable;
        self
    }

    /// Build the configuration
    pub fn build(self) -> BenchConfig {
        let nodes = if self.nodes.is_empty() {
            vec![NodeConfig::default()]
        } else {
            self.nodes
        };

        BenchConfig {
            contract_config_path: self
                .contract_config_path
                .unwrap_or_else(|| PathBuf::from("deploy.json")),
            target_tps: self.target_tps.unwrap_or(10000),
            nodes,
            num_tokens: self.num_tokens.unwrap_or(2),
            enable_swap_token: self.enable_swap_token.unwrap_or(false),
            faucet: self.faucet.unwrap_or_else(FaucetConfig::test_default),
            accounts: self.accounts.unwrap_or_default(),
            performance: self.performance.unwrap_or_default(),
            recovery_mode: self.recovery_mode,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_config_default() {
        let config = NodeConfig::default();
        assert_eq!(config.rpc_url, "http://localhost:8545");
        assert_eq!(config.chain_id, 7771625);
    }

    #[test]
    fn test_node_config_localhost_with_port() {
        let config = NodeConfig::localhost_with_port(9545, 1);
        assert_eq!(config.rpc_url, "http://localhost:9545");
        assert_eq!(config.chain_id, 1);
    }

    #[test]
    fn test_faucet_config() {
        let config = FaucetConfig::new("test_key")
            .with_faucet_level(5)
            .with_wait_duration(30);

        assert_eq!(config.private_key, "test_key");
        assert_eq!(config.faucet_level, 5);
        assert_eq!(config.wait_duration_secs, 30);
    }

    #[test]
    fn test_bench_config_builder() {
        let config = BenchConfig::builder()
            .target_tps(5000)
            .rpc_url("http://localhost:8545")
            .chain_id(7771625)
            .num_accounts(500)
            .duration_secs(30)
            .build();

        assert_eq!(config.target_tps, 5000);
        assert_eq!(config.accounts.num_accounts, 500);
        assert_eq!(config.performance.duration_secs, 30);
    }

    #[test]
    fn test_bench_config_to_toml() {
        let config = BenchConfig::builder()
            .target_tps(1000)
            .build();

        let toml = config.to_toml();
        assert!(toml.contains("target_tps = 1000"));
        assert!(toml.contains("[faucet]"));
        assert!(toml.contains("[accounts]"));
        assert!(toml.contains("[performance]"));
    }
}

