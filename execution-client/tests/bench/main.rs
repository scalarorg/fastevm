//! FastEVM Benchmark Tool
//!
//! A custom benchmark binary that integrates gravity_bench with FastEVM.
//! This tool can start a FastEVM node and run benchmarks against it.
//!
//! # Usage
//!
//! ```sh
//! # Run benchmark against existing node
//! fastevm-bench --rpc-url http://localhost:8545
//!
//! # Run with custom configuration
//! fastevm-bench --rpc-url http://localhost:8545 --target-tps 5000 --duration 300
//!
//! # Run with config file
//! fastevm-bench --config bench.toml
//! ```

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::time::Duration;

/// FastEVM Benchmark Tool
#[derive(Parser, Debug)]
#[command(name = "fastevm-bench")]
#[command(
    author,
    version,
    about = "FastEVM Benchmark Tool - Transaction generator and performance tester"
)]
pub struct BenchCli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run benchmark against an existing node
    Run(RunArgs),

    /// Generate a benchmark configuration file
    Config(ConfigArgs),

    /// Check node health and readiness
    Health(HealthArgs),

    /// Show benchmark statistics from a previous run
    Stats(StatsArgs),
}

#[derive(Parser, Debug)]
struct RunArgs {
    /// RPC URL of the node to benchmark
    #[arg(long, default_value = "http://localhost:8545")]
    rpc_url: String,

    /// Chain ID
    #[arg(long, default_value = "7771625")]
    chain_id: u64,

    /// Target transactions per second
    #[arg(long, default_value = "1000")]
    target_tps: u32,

    /// Benchmark duration in seconds
    #[arg(long, default_value = "60")]
    duration: u64,

    /// Number of accounts to use for sending transactions
    #[arg(long, default_value = "100")]
    num_accounts: u32,

    /// Number of concurrent senders
    #[arg(long, default_value = "50")]
    num_senders: u32,

    /// Faucet private key (hex format with 0x prefix)
    #[arg(long, env = "FAUCET_PRIVATE_KEY")]
    faucet_key: Option<String>,

    /// Path to configuration file (overrides CLI args)
    #[arg(long, short)]
    config: Option<PathBuf>,

    /// Working directory for benchmark data
    #[arg(long, default_value = "./bench_data")]
    data_dir: PathBuf,

    /// Enable swap token transactions
    #[arg(long)]
    enable_swap: bool,

    /// Number of tokens for swap transactions
    #[arg(long, default_value = "2")]
    num_tokens: u32,

    /// Recovery mode - resume from previous state
    #[arg(long)]
    recover: bool,

    /// Verbose output
    #[arg(long, short)]
    verbose: bool,
}

#[derive(Parser, Debug)]
struct ConfigArgs {
    /// Output path for the configuration file
    #[arg(long, short, default_value = "bench.toml")]
    output: PathBuf,

    /// Use high throughput preset
    #[arg(long)]
    high_throughput: bool,

    /// Use low latency preset
    #[arg(long)]
    low_latency: bool,

    /// Target TPS for custom config
    #[arg(long)]
    target_tps: Option<u32>,

    /// Duration in seconds
    #[arg(long)]
    duration: Option<u64>,
}

#[derive(Parser, Debug)]
struct HealthArgs {
    /// RPC URL of the node to check
    #[arg(long, default_value = "http://localhost:8545")]
    rpc_url: String,

    /// Timeout in seconds
    #[arg(long, default_value = "10")]
    timeout: u64,
}

#[derive(Parser, Debug)]
struct StatsArgs {
    /// Path to stats file
    #[arg(long, default_value = "./bench_data/stats.json")]
    path: PathBuf,
}

/// Benchmark configuration
#[derive(Debug, Clone)]
pub struct BenchConfig {
    pub rpc_url: String,
    pub chain_id: u64,
    pub target_tps: u32,
    pub duration_secs: u64,
    pub num_accounts: u32,
    pub num_senders: u32,
    pub faucet_key: Option<String>,
    pub enable_swap: bool,
    pub num_tokens: u32,
    pub recover: bool,
    pub data_dir: PathBuf,
}

impl Default for BenchConfig {
    fn default() -> Self {
        Self {
            rpc_url: "http://localhost:8545".to_string(),
            chain_id: 7771625,
            target_tps: 1000,
            duration_secs: 60,
            num_accounts: 100,
            num_senders: 50,
            faucet_key: None,
            enable_swap: false,
            num_tokens: 2,
            recover: false,
            data_dir: PathBuf::from("./bench_data"),
        }
    }
}

impl BenchConfig {
    /// Create config from CLI arguments
    fn from_run_args(args: &RunArgs) -> Self {
        Self {
            rpc_url: args.rpc_url.clone(),
            chain_id: args.chain_id,
            target_tps: args.target_tps,
            duration_secs: args.duration,
            num_accounts: args.num_accounts,
            num_senders: args.num_senders,
            faucet_key: args.faucet_key.clone(),
            enable_swap: args.enable_swap,
            num_tokens: args.num_tokens,
            recover: args.recover,
            data_dir: args.data_dir.clone(),
        }
    }

    /// High throughput preset for stress testing
    pub fn high_throughput() -> Self {
        Self {
            target_tps: 10000,
            num_accounts: 1000,
            num_senders: 200,
            duration_secs: 300,
            ..Default::default()
        }
    }

    /// Low latency preset for latency testing
    pub fn low_latency() -> Self {
        Self {
            target_tps: 100,
            num_accounts: 10,
            num_senders: 5,
            duration_secs: 60,
            ..Default::default()
        }
    }

    /// Generate TOML configuration string
    pub fn to_toml(&self) -> String {
        format!(
            r#"# FastEVM Benchmark Configuration
# Generated by fastevm-bench

target_tps = {}
enable_swap_token = {}
num_tokens = {}
recovery_mode = {}

[[nodes]]
rpc_url = "{}"
chain_id = {}

[faucet]
private_key = "{}"

[accounts]
num_accounts = {}

[performance]
num_senders = {}
duration_secs = {}
max_pool_size = {}
"#,
            self.target_tps,
            self.enable_swap,
            self.num_tokens,
            self.recover,
            self.rpc_url,
            self.chain_id,
            self.faucet_key.as_deref().unwrap_or("0x..."),
            self.num_accounts,
            self.num_senders,
            self.duration_secs,
            self.target_tps * 10, // max_pool_size = 10x target TPS
        )
    }

    /// Write configuration to file
    pub fn write_to_file(&self, path: &PathBuf) -> std::io::Result<()> {
        std::fs::write(path, self.to_toml())
    }
}

/// Benchmark statistics
#[derive(Debug, Clone, Default)]
pub struct BenchStats {
    pub total_txs_sent: u64,
    pub total_txs_confirmed: u64,
    pub avg_tps: f64,
    pub peak_tps: f64,
    pub avg_latency_ms: f64,
    pub p99_latency_ms: f64,
    pub duration_secs: u64,
    pub blocks_produced: u64,
    pub start_block: u64,
    pub end_block: u64,
}

impl BenchStats {
    /// Print formatted statistics
    pub fn print(&self) {
        println!("\n========================================");
        println!("  Benchmark Results");
        println!("========================================");
        println!("  Duration:         {} seconds", self.duration_secs);
        println!("  Blocks produced:  {}", self.blocks_produced);
        println!(
            "  Block range:      {} - {}",
            self.start_block, self.end_block
        );
        println!("  ──────────────────────────────────────");
        println!("  TXs sent:         {}", self.total_txs_sent);
        println!("  TXs confirmed:    {}", self.total_txs_confirmed);
        println!("  Average TPS:      {:.2}", self.avg_tps);
        println!("  Peak TPS:         {:.2}", self.peak_tps);
        println!("  ──────────────────────────────────────");
        println!("  Avg latency:      {:.2} ms", self.avg_latency_ms);
        println!("  P99 latency:      {:.2} ms", self.p99_latency_ms);
        println!("========================================\n");
    }
}

/// RPC client for node interaction
struct RpcClient {
    client: reqwest::Client,
    rpc_url: String,
}

impl RpcClient {
    fn new(rpc_url: &str) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
            rpc_url: rpc_url.to_string(),
        }
    }

    async fn call(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": 1
        });

        let response = self
            .client
            .post(&self.rpc_url)
            .json(&request)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        if let Some(error) = json.get("error") {
            return Err(format!("RPC error: {}", error));
        }

        Ok(json
            .get("result")
            .cloned()
            .unwrap_or(serde_json::Value::Null))
    }

    async fn get_block_number(&self) -> Result<u64, String> {
        let result = self.call("eth_blockNumber", serde_json::json!([])).await?;
        let hex = result.as_str().unwrap_or("0x0");
        u64::from_str_radix(hex.trim_start_matches("0x"), 16)
            .map_err(|e| format!("Parse error: {}", e))
    }

    async fn get_chain_id(&self) -> Result<u64, String> {
        let result = self.call("eth_chainId", serde_json::json!([])).await?;
        let hex = result.as_str().unwrap_or("0x0");
        u64::from_str_radix(hex.trim_start_matches("0x"), 16)
            .map_err(|e| format!("Parse error: {}", e))
    }

    async fn is_syncing(&self) -> Result<bool, String> {
        let result = self.call("eth_syncing", serde_json::json!([])).await?;
        Ok(!result.is_boolean() || result.as_bool() == Some(true))
    }

    async fn count_txs_in_block(&self, block_num: u64) -> Result<u64, String> {
        let hex_block = format!("0x{:x}", block_num);
        let result = self
            .call(
                "eth_getBlockByNumber",
                serde_json::json!([hex_block, false]),
            )
            .await?;

        if result.is_null() {
            return Ok(0);
        }

        let txs = result
            .get("transactions")
            .and_then(|t| t.as_array())
            .map(|a| a.len() as u64)
            .unwrap_or(0);

        Ok(txs)
    }
}

/// Run the benchmark
async fn run_benchmark(args: RunArgs) -> Result<(), String> {
    println!("\n========================================");
    println!("  FastEVM Benchmark");
    println!("========================================\n");

    let config = BenchConfig::from_run_args(&args);

    // Create data directory
    std::fs::create_dir_all(&config.data_dir)
        .map_err(|e| format!("Failed to create data directory: {}", e))?;

    println!("Configuration:");
    println!("  RPC URL:      {}", config.rpc_url);
    println!("  Chain ID:     {}", config.chain_id);
    println!("  Target TPS:   {}", config.target_tps);
    println!("  Duration:     {} seconds", config.duration_secs);
    println!("  Accounts:     {}", config.num_accounts);
    println!("  Senders:      {}", config.num_senders);
    println!("  Data dir:     {}", config.data_dir.display());
    println!();

    // Check node health
    println!("[1/4] Checking node health...");
    let rpc = RpcClient::new(&config.rpc_url);

    let chain_id = rpc.get_chain_id().await?;
    println!("  ✓ Connected to node (chain_id: {})", chain_id);

    let start_block = rpc.get_block_number().await?;
    println!("  ✓ Current block: {}", start_block);

    if rpc.is_syncing().await? {
        println!("  ⚠ Node is syncing, results may be affected");
    }

    // Write config file for gravity_bench
    println!("\n[2/4] Preparing benchmark configuration...");
    let config_path = config.data_dir.join("bench_config.toml");
    config
        .write_to_file(&config_path)
        .map_err(|e| format!("Failed to write config: {}", e))?;
    println!("  ✓ Config written to: {}", config_path.display());

    // Run gravity_bench
    println!(
        "\n[3/4] Running benchmark for {} seconds...",
        config.duration_secs
    );
    println!("  Target TPS: {}", config.target_tps);
    println!();

    // Try to run gravity_bench binary
    let gravity_bench_result = std::process::Command::new("gravity_bench")
        .arg("--config")
        .arg(&config_path)
        .current_dir(&config.data_dir)
        .output();

    let mut stats = BenchStats::default();
    stats.duration_secs = config.duration_secs;
    stats.start_block = start_block;

    match gravity_bench_result {
        Ok(output) => {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                println!("{}", stdout);

                // Parse stats from output
                for line in stdout.lines() {
                    if line.contains("Total TXs:") {
                        if let Some(num) = extract_number(line) {
                            stats.total_txs_sent = num;
                        }
                    }
                    if line.contains("TPS:") && !line.contains("Peak") {
                        if let Some(num) = extract_float(line) {
                            stats.avg_tps = num;
                        }
                    }
                    if line.contains("Peak TPS:") {
                        if let Some(num) = extract_float(line) {
                            stats.peak_tps = num;
                        }
                    }
                }
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                println!("  ⚠ gravity_bench failed: {}", stderr);
                println!("  Running simulation mode instead...");

                // Simulate benchmark for demo purposes
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
        Err(_) => {
            println!("  ⚠ gravity_bench not found in PATH");
            println!("  Install from: https://github.com/Galxe/gravity_bench.git");
            println!();
            println!(
                "  Running in simulation mode (waiting {} seconds)...",
                config.duration_secs.min(10)
            );

            // Wait for configured duration (capped at 10s for simulation)
            tokio::time::sleep(Duration::from_secs(config.duration_secs.min(10))).await;
        }
    }

    // Collect final stats
    println!("\n[4/4] Collecting statistics...");
    let end_block = rpc.get_block_number().await?;
    stats.end_block = end_block;
    stats.blocks_produced = end_block.saturating_sub(start_block);

    // Count transactions in blocks
    let mut total_txs = 0u64;
    for block_num in (start_block + 1)..=end_block {
        total_txs += rpc.count_txs_in_block(block_num).await.unwrap_or(0);
    }
    stats.total_txs_confirmed = total_txs;

    if stats.total_txs_sent == 0 {
        stats.total_txs_sent = total_txs;
    }

    if stats.avg_tps == 0.0 && config.duration_secs > 0 {
        stats.avg_tps = total_txs as f64 / config.duration_secs as f64;
    }

    stats.print();

    // Save stats to file
    let stats_path = config.data_dir.join("stats.json");
    let stats_json = serde_json::json!({
        "total_txs_sent": stats.total_txs_sent,
        "total_txs_confirmed": stats.total_txs_confirmed,
        "avg_tps": stats.avg_tps,
        "peak_tps": stats.peak_tps,
        "avg_latency_ms": stats.avg_latency_ms,
        "duration_secs": stats.duration_secs,
        "blocks_produced": stats.blocks_produced,
        "start_block": stats.start_block,
        "end_block": stats.end_block,
    });

    std::fs::write(
        &stats_path,
        serde_json::to_string_pretty(&stats_json).unwrap(),
    )
    .map_err(|e| format!("Failed to write stats: {}", e))?;
    println!("Stats saved to: {}", stats_path.display());

    Ok(())
}

/// Generate configuration file
fn generate_config(args: ConfigArgs) -> Result<(), String> {
    let config = if args.high_throughput {
        println!("Generating high throughput configuration...");
        BenchConfig::high_throughput()
    } else if args.low_latency {
        println!("Generating low latency configuration...");
        BenchConfig::low_latency()
    } else {
        println!("Generating default configuration...");
        let mut config = BenchConfig::default();
        if let Some(tps) = args.target_tps {
            config.target_tps = tps;
        }
        if let Some(duration) = args.duration {
            config.duration_secs = duration;
        }
        config
    };

    config
        .write_to_file(&args.output)
        .map_err(|e| format!("Failed to write config: {}", e))?;

    println!("Configuration written to: {}", args.output.display());
    println!("\nEdit the file to customize settings, then run:");
    println!("  fastevm-bench run --config {}", args.output.display());

    Ok(())
}

/// Check node health
async fn check_health(args: HealthArgs) -> Result<(), String> {
    println!("Checking node health at {}...\n", args.rpc_url);

    let rpc = RpcClient::new(&args.rpc_url);

    let timeout = Duration::from_secs(args.timeout);
    let start = std::time::Instant::now();

    while start.elapsed() < timeout {
        match rpc.get_block_number().await {
            Ok(block) => {
                let chain_id = rpc.get_chain_id().await.unwrap_or(0);
                let syncing = rpc.is_syncing().await.unwrap_or(false);

                println!("✓ Node is healthy");
                println!("  Block number: {}", block);
                println!("  Chain ID:     {}", chain_id);
                println!("  Syncing:      {}", if syncing { "yes" } else { "no" });
                return Ok(());
            }
            Err(_) => {
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
    }

    Err(format!("Node not reachable after {} seconds", args.timeout))
}

/// Show statistics from previous run
fn show_stats(args: StatsArgs) -> Result<(), String> {
    let content = std::fs::read_to_string(&args.path)
        .map_err(|e| format!("Failed to read stats file: {}", e))?;

    let json: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| format!("Failed to parse stats: {}", e))?;

    println!("\n========================================");
    println!("  Benchmark Statistics");
    println!("========================================");
    println!("  File: {}", args.path.display());
    println!();
    println!("{}", serde_json::to_string_pretty(&json).unwrap());

    Ok(())
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

#[tokio::main]
async fn main() {
    let cli = BenchCli::parse();

    let result = match cli.command {
        Commands::Run(args) => run_benchmark(args).await,
        Commands::Config(args) => generate_config(args),
        Commands::Health(args) => check_health(args).await,
        Commands::Stats(args) => show_stats(args),
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
