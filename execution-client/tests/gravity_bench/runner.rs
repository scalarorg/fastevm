//! Runner module for gravity_bench integration
//!
//! This module provides the benchmark runner that orchestrates
//! the execution of gravity_bench against FastEVM nodes.

use super::config::BenchConfig;
use std::fs::File;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

/// Result type for benchmark operations
pub type BenchResult<T> = Result<T, BenchError>;

/// Error types for benchmark operations
#[derive(Debug)]
pub enum BenchError {
    /// Configuration error
    ConfigError(String),
    /// Process spawn error
    SpawnError(String),
    /// Process execution error
    ExecutionError(String),
    /// Timeout error
    TimeoutError(String),
    /// IO error
    IoError(std::io::Error),
    /// Node not ready
    NodeNotReady(String),
}

impl std::fmt::Display for BenchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BenchError::ConfigError(msg) => write!(f, "Configuration error: {}", msg),
            BenchError::SpawnError(msg) => write!(f, "Spawn error: {}", msg),
            BenchError::ExecutionError(msg) => write!(f, "Execution error: {}", msg),
            BenchError::TimeoutError(msg) => write!(f, "Timeout error: {}", msg),
            BenchError::IoError(e) => write!(f, "IO error: {}", e),
            BenchError::NodeNotReady(msg) => write!(f, "Node not ready: {}", msg),
        }
    }
}

impl std::error::Error for BenchError {}

impl From<std::io::Error> for BenchError {
    fn from(e: std::io::Error) -> Self {
        BenchError::IoError(e)
    }
}

/// Benchmark statistics collected during the run
#[derive(Debug, Clone, Default)]
pub struct BenchStats {
    /// Total transactions sent
    pub total_txs_sent: u64,
    /// Total transactions confirmed
    pub total_txs_confirmed: u64,
    /// Average TPS achieved
    pub avg_tps: f64,
    /// Peak TPS achieved
    pub peak_tps: f64,
    /// Average latency in milliseconds
    pub avg_latency_ms: f64,
    /// Duration of the benchmark in seconds
    pub duration_secs: u64,
    /// Number of failed transactions
    pub failed_txs: u64,
}

/// Handle for a running FastEVM node process
pub struct NodeHandle {
    process: Option<Child>,
    rpc_url: String,
    data_dir: PathBuf,
    log_file: Option<PathBuf>,
}

impl NodeHandle {
    /// Get the RPC URL of the node
    pub fn rpc_url(&self) -> &str {
        &self.rpc_url
    }

    /// Get the data directory of the node
    pub fn data_dir(&self) -> &PathBuf {
        &self.data_dir
    }

    /// Get the log file path
    pub fn log_file(&self) -> Option<&PathBuf> {
        self.log_file.as_ref()
    }

    /// Check if the node is running
    pub fn is_running(&self) -> bool {
        self.process.is_some()
    }

    /// Stop the node gracefully
    pub fn stop(&mut self) -> BenchResult<()> {
        if let Some(mut process) = self.process.take() {
            // Kill the process
            process.kill().map_err(|e| {
                BenchError::ExecutionError(format!("Failed to kill node process: {}", e))
            })?;

            // Wait for process to exit
            process.wait().map_err(|e| {
                BenchError::ExecutionError(format!("Failed to wait for node process: {}", e))
            })?;

            println!("Node stopped successfully");
        }
        Ok(())
    }

    /// Clean up node data directory
    pub fn cleanup(&self) -> BenchResult<()> {
        if self.data_dir.exists() {
            std::fs::remove_dir_all(&self.data_dir).map_err(|e| BenchError::IoError(e))?;
        }
        Ok(())
    }
}

impl Drop for NodeHandle {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

/// RPC client for interacting with the node
pub struct RpcClient {
    rpc_url: String,
    client: reqwest::Client,
}

impl RpcClient {
    /// Create a new RPC client
    pub fn new(rpc_url: impl Into<String>) -> Self {
        Self {
            rpc_url: rpc_url.into(),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
        }
    }

    /// Make an RPC call
    async fn call(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> BenchResult<serde_json::Value> {
        let request_body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": 1
        });

        let response = self
            .client
            .post(&self.rpc_url)
            .json(&request_body)
            .send()
            .await
            .map_err(|e| BenchError::ExecutionError(format!("RPC call failed: {}", e)))?;

        let json: serde_json::Value = response.json().await.map_err(|e| {
            BenchError::ExecutionError(format!("Failed to parse RPC response: {}", e))
        })?;

        if let Some(error) = json.get("error") {
            return Err(BenchError::ExecutionError(format!("RPC error: {}", error)));
        }

        Ok(json
            .get("result")
            .cloned()
            .unwrap_or(serde_json::Value::Null))
    }

    /// Get the current block number
    pub async fn get_block_number(&self) -> BenchResult<u64> {
        let result = self.call("eth_blockNumber", serde_json::json!([])).await?;
        let hex_str = result.as_str().unwrap_or("0x0");
        let block_num = u64::from_str_radix(hex_str.trim_start_matches("0x"), 16).map_err(|e| {
            BenchError::ExecutionError(format!("Failed to parse block number: {}", e))
        })?;
        Ok(block_num)
    }

    /// Get account balance in wei
    pub async fn get_balance(&self, address: &str) -> BenchResult<u128> {
        let result = self
            .call("eth_getBalance", serde_json::json!([address, "latest"]))
            .await?;
        let hex_str = result.as_str().unwrap_or("0x0");
        let balance = u128::from_str_radix(hex_str.trim_start_matches("0x"), 16)
            .map_err(|e| BenchError::ExecutionError(format!("Failed to parse balance: {}", e)))?;
        Ok(balance)
    }

    /// Get transaction count for an account
    pub async fn get_transaction_count(&self, address: &str) -> BenchResult<u64> {
        let result = self
            .call(
                "eth_getTransactionCount",
                serde_json::json!([address, "latest"]),
            )
            .await?;
        let hex_str = result.as_str().unwrap_or("0x0");
        let count = u64::from_str_radix(hex_str.trim_start_matches("0x"), 16)
            .map_err(|e| BenchError::ExecutionError(format!("Failed to parse tx count: {}", e)))?;
        Ok(count)
    }

    /// Get block by number
    pub async fn get_block_by_number(&self, block_num: u64) -> BenchResult<serde_json::Value> {
        let hex_block = format!("0x{:x}", block_num);
        self.call("eth_getBlockByNumber", serde_json::json!([hex_block, true]))
            .await
    }

    /// Get transaction receipt
    pub async fn get_transaction_receipt(
        &self,
        tx_hash: &str,
    ) -> BenchResult<Option<serde_json::Value>> {
        let result = self
            .call("eth_getTransactionReceipt", serde_json::json!([tx_hash]))
            .await?;
        if result.is_null() {
            Ok(None)
        } else {
            Ok(Some(result))
        }
    }

    /// Check if node is syncing
    pub async fn is_syncing(&self) -> BenchResult<bool> {
        let result = self.call("eth_syncing", serde_json::json!([])).await?;
        Ok(!result.is_boolean() || result.as_bool() == Some(true))
    }

    /// Get chain ID
    pub async fn get_chain_id(&self) -> BenchResult<u64> {
        let result = self.call("eth_chainId", serde_json::json!([])).await?;
        let hex_str = result.as_str().unwrap_or("0x0");
        let chain_id = u64::from_str_radix(hex_str.trim_start_matches("0x"), 16)
            .map_err(|e| BenchError::ExecutionError(format!("Failed to parse chain ID: {}", e)))?;
        Ok(chain_id)
    }

    /// Count total transactions in a range of blocks
    pub async fn count_transactions_in_blocks(
        &self,
        from_block: u64,
        to_block: u64,
    ) -> BenchResult<u64> {
        let mut total_txs = 0u64;

        for block_num in from_block..=to_block {
            let block = self.get_block_by_number(block_num).await?;
            if let Some(txs) = block.get("transactions") {
                if let Some(arr) = txs.as_array() {
                    total_txs += arr.len() as u64;
                }
            }
        }

        Ok(total_txs)
    }
}

/// Benchmark runner that orchestrates the benchmark execution
pub struct BenchRunner {
    config: BenchConfig,
    gravity_bench_path: Option<PathBuf>,
    node_binary_path: Option<PathBuf>,
    working_dir: PathBuf,
}

impl BenchRunner {
    /// Create a new benchmark runner with the given configuration
    pub fn new(config: BenchConfig) -> Self {
        Self {
            config,
            gravity_bench_path: None,
            node_binary_path: None,
            working_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }

    /// Set the path to the gravity_bench binary
    pub fn with_gravity_bench_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.gravity_bench_path = Some(path.into());
        self
    }

    /// Set the path to the FastEVM node binary
    pub fn with_node_binary_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.node_binary_path = Some(path.into());
        self
    }

    /// Set the working directory
    pub fn with_working_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.working_dir = path.into();
        self
    }

    /// Get the configuration
    pub fn config(&self) -> &BenchConfig {
        &self.config
    }

    /// Start a FastEVM gravity node for testing
    /// This uses the fastevm-gravity binary which is optimized for benchmarking
    pub fn start_gravity_node(&self, port: u16) -> BenchResult<NodeHandle> {
        let node_binary = self
            .node_binary_path
            .clone()
            .unwrap_or_else(|| PathBuf::from("target/release/fastevm-gravity"));

        println!(
            "Starting fastevm-gravity node from: {}",
            node_binary.display()
        );

        if !node_binary.exists() {
            return Err(BenchError::ConfigError(format!(
                "Node binary not found at: {}. Run 'cargo build --release --bin fastevm-gravity' first.",
                node_binary.display()
            )));
        }

        let data_dir = self.working_dir.join(format!("bench_data_{}", port));
        let log_file = self
            .working_dir
            .join(format!("fastevm-gravity-{}.log", port));

        // Clean up previous data if exists
        if data_dir.exists() {
            std::fs::remove_dir_all(&data_dir)?;
        }
        std::fs::create_dir_all(&data_dir)?;

        // Open log file for output
        let log_output = File::create(&log_file)?;
        let log_err = log_output.try_clone()?;

        let process = Command::new(&node_binary)
            .arg("node")
            .arg("--datadir")
            .arg(&data_dir)
            .arg("--http")
            .arg("--http.addr")
            .arg("0.0.0.0")
            .arg("--http.port")
            .arg(port.to_string())
            .arg("--http.api")
            .arg("eth,net,web3,debug,trace")
            .stdout(Stdio::from(log_output))
            .stderr(Stdio::from(log_err))
            .spawn()
            .map_err(|e| BenchError::SpawnError(format!("Failed to spawn gravity node: {}", e)))?;

        let rpc_url = format!("http://localhost:{}", port);

        println!("Node started with PID: {}", process.id());
        println!("RPC URL: {}", rpc_url);
        println!("Log file: {}", log_file.display());

        Ok(NodeHandle {
            process: Some(process),
            rpc_url,
            data_dir,
            log_file: Some(log_file),
        })
    }

    /// Start a FastEVM execution node for testing (original method)
    pub fn start_node(&self, port: u16) -> BenchResult<NodeHandle> {
        let node_binary = self
            .node_binary_path
            .clone()
            .unwrap_or_else(|| PathBuf::from("target/debug/fastevm-execution"));

        if !node_binary.exists() {
            return Err(BenchError::ConfigError(format!(
                "Node binary not found at: {}",
                node_binary.display()
            )));
        }

        let data_dir = self.working_dir.join(format!("test_data_{}", port));
        let log_file = self.working_dir.join(format!("fastevm-{}.log", port));
        std::fs::create_dir_all(&data_dir)?;

        let log_output = File::create(&log_file)?;
        let log_err = log_output.try_clone()?;

        let process = Command::new(&node_binary)
            .arg("node")
            .arg("--chain")
            .arg("dev")
            .arg("--datadir")
            .arg(&data_dir)
            .arg("--http")
            .arg("--http.port")
            .arg(port.to_string())
            .arg("--enable-tx-subscription")
            .arg("--committed-subdags-per-block")
            .arg("1")
            .arg("--block-build-interval-ms")
            .arg("100")
            .stdout(Stdio::from(log_output))
            .stderr(Stdio::from(log_err))
            .spawn()
            .map_err(|e| BenchError::SpawnError(format!("Failed to spawn node: {}", e)))?;

        let rpc_url = format!("http://localhost:{}", port);

        Ok(NodeHandle {
            process: Some(process),
            rpc_url,
            data_dir,
            log_file: Some(log_file),
        })
    }

    /// Create an RPC client for the given URL
    pub fn create_rpc_client(&self, rpc_url: &str) -> RpcClient {
        RpcClient::new(rpc_url)
    }

    /// Wait for a node to be ready by checking the RPC endpoint
    pub async fn wait_for_node_ready(&self, rpc_url: &str, timeout: Duration) -> BenchResult<()> {
        let start = std::time::Instant::now();

        while start.elapsed() < timeout {
            if self.check_node_health(rpc_url).await {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        Err(BenchError::TimeoutError(format!(
            "Node at {} not ready after {:?}",
            rpc_url, timeout
        )))
    }

    /// Check if a node is healthy by making an RPC call
    async fn check_node_health(&self, rpc_url: &str) -> bool {
        // Simple HTTP check - in production, use proper RPC client
        let client = match reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
        {
            Ok(c) => c,
            Err(_) => return false,
        };

        let request_body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_blockNumber",
            "params": [],
            "id": 1
        });

        match client.post(rpc_url).json(&request_body).send().await {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
    }

    /// Run the benchmark using gravity_bench
    pub async fn run_benchmark(&self) -> BenchResult<BenchStats> {
        // Write config to temporary file
        let config_path = self.working_dir.join("bench_config_test.toml");
        self.config.write_to_file(&config_path)?;

        let gravity_bench = self
            .gravity_bench_path
            .clone()
            .unwrap_or_else(|| PathBuf::from("gravity_bench"));

        // Build command arguments
        let mut args = vec!["--config".to_string(), config_path.display().to_string()];

        if self.config.recovery_mode {
            args.push("--recover".to_string());
        }

        // Run gravity_bench
        let output = Command::new(&gravity_bench)
            .args(&args)
            .current_dir(&self.working_dir)
            .output()
            .map_err(|e| BenchError::SpawnError(format!("Failed to run gravity_bench: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(BenchError::ExecutionError(format!(
                "gravity_bench failed: {}",
                stderr
            )));
        }

        // Parse output and extract stats
        let stdout = String::from_utf8_lossy(&output.stdout);
        self.parse_bench_output(&stdout)
    }

    /// Parse the gravity_bench output to extract statistics
    fn parse_bench_output(&self, output: &str) -> BenchResult<BenchStats> {
        let mut stats = BenchStats::default();
        stats.duration_secs = self.config.performance.duration_secs;

        // Parse common metrics from gravity_bench output
        for line in output.lines() {
            if line.contains("Total TXs:") {
                if let Some(num) = extract_number(line) {
                    stats.total_txs_sent = num;
                }
            }
            if line.contains("TPS:") || line.contains("tps:") {
                if let Some(num) = extract_float(line) {
                    stats.avg_tps = num;
                }
            }
            if line.contains("Peak TPS:") {
                if let Some(num) = extract_float(line) {
                    stats.peak_tps = num;
                }
            }
            if line.contains("Latency:") || line.contains("latency:") {
                if let Some(num) = extract_float(line) {
                    stats.avg_latency_ms = num;
                }
            }
        }

        Ok(stats)
    }

    /// Run a quick smoke test
    pub async fn smoke_test(&self) -> BenchResult<bool> {
        // Check that node is reachable
        for node in &self.config.nodes {
            if !self.check_node_health(&node.rpc_url).await {
                return Err(BenchError::NodeNotReady(format!(
                    "Node at {} is not reachable",
                    node.rpc_url
                )));
            }
        }

        Ok(true)
    }
}

/// Extract a number from a string
fn extract_number(s: &str) -> Option<u64> {
    s.split_whitespace().find_map(|word| {
        word.trim_matches(|c: char| !c.is_ascii_digit())
            .parse()
            .ok()
    })
}

/// Extract a floating point number from a string
fn extract_float(s: &str) -> Option<f64> {
    s.split_whitespace().find_map(|word| {
        word.trim_matches(|c: char| !c.is_ascii_digit() && c != '.')
            .parse()
            .ok()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bench_error_display() {
        let error = BenchError::ConfigError("test error".to_string());
        assert!(error.to_string().contains("Configuration error"));
    }

    #[test]
    fn test_bench_stats_default() {
        let stats = BenchStats::default();
        assert_eq!(stats.total_txs_sent, 0);
        assert_eq!(stats.avg_tps, 0.0);
    }

    #[test]
    fn test_extract_number() {
        assert_eq!(extract_number("Total TXs: 1000"), Some(1000));
        assert_eq!(extract_number("no number here"), None);
    }

    #[test]
    fn test_extract_float() {
        assert_eq!(extract_float("TPS: 1234.56"), Some(1234.56));
        assert_eq!(extract_float("no float here"), None);
    }

    #[test]
    fn test_bench_runner_new() {
        let config = BenchConfig::builder().build();
        let runner = BenchRunner::new(config);

        assert!(runner.node_binary_path.is_none());
        assert!(runner.gravity_bench_path.is_none());
    }

    #[test]
    fn test_bench_runner_with_paths() {
        let config = BenchConfig::builder().build();
        let runner = BenchRunner::new(config)
            .with_gravity_bench_path("/path/to/bench")
            .with_node_binary_path("/path/to/node");

        assert_eq!(
            runner.gravity_bench_path,
            Some(PathBuf::from("/path/to/bench"))
        );
        assert_eq!(
            runner.node_binary_path,
            Some(PathBuf::from("/path/to/node"))
        );
    }
}
