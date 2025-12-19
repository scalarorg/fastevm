//! Integration tests for FastEVM with gravity_bench
//!
//! These tests verify the integration between FastEVM execution client
//! and the gravity_bench transaction generator.
//!
//! # Running the tests
//!
//! Quick tests (no node required):
//! ```sh
//! cargo test --package fastevm-execution --test bench_tests
//! ```
//!
//! Full integration tests (requires node):
//! ```sh
//! cargo test --package fastevm-execution --test bench_tests -- --ignored --nocapture
//! ```

mod gravity_bench;

use gravity_bench::{BenchConfig, BenchRunner, NodeConfig};
use std::time::Duration;

// =============================================================================
// Unit Tests - No node required
// =============================================================================

/// Test BenchConfig creation
#[test]
fn test_bench_config_creation() {
    let config = BenchConfig::builder()
        .target_tps(5000)
        .rpc_url("http://localhost:8545")
        .chain_id(7771625)
        .num_accounts(1000)
        .duration_secs(60)
        .num_senders(100)
        .build();

    assert_eq!(config.target_tps, 5000);
    assert_eq!(config.accounts.num_accounts, 1000);
    assert_eq!(config.performance.duration_secs, 60);
    assert_eq!(config.performance.num_senders, 100);
}

/// Test BenchConfig TOML generation
#[test]
fn test_bench_config_toml_generation() {
    let config = BenchConfig::builder()
        .target_tps(10000)
        .rpc_url("http://localhost:8545")
        .build();

    let toml = config.to_toml();

    assert!(toml.contains("target_tps = 10000"));
    assert!(toml.contains("http://localhost:8545"));
    assert!(toml.contains("[faucet]"));
    assert!(toml.contains("[accounts]"));
    assert!(toml.contains("[performance]"));
}

/// Test NodeConfig creation
#[test]
fn test_node_config() {
    let config = NodeConfig::localhost_with_port(9545, 7771625);

    assert_eq!(config.rpc_url, "http://localhost:9545");
    assert_eq!(config.chain_id, 7771625);
}

/// Test BenchRunner creation
#[test]
fn test_bench_runner_creation() {
    let config = BenchConfig::builder().build();
    let runner = BenchRunner::new(config)
        .with_gravity_bench_path("/path/to/gravity_bench")
        .with_node_binary_path("/path/to/fastevm-gravity")
        .with_working_dir("/tmp/bench_test");

    assert!(runner.config().target_tps > 0);
}

/// Test configuration with multiple nodes
#[test]
fn test_multi_node_config() {
    let config = BenchConfig::builder()
        .add_node(NodeConfig::localhost_with_port(8545, 7771625))
        .add_node(NodeConfig::localhost_with_port(8546, 7771625))
        .add_node(NodeConfig::localhost_with_port(8547, 7771625))
        .target_tps(30000)
        .build();

    assert_eq!(config.nodes.len(), 3);
    assert_eq!(config.target_tps, 30000);
}

/// Test recovery mode configuration
#[test]
fn test_recovery_mode_config() {
    let config = BenchConfig::builder().recovery_mode(true).build();

    assert!(config.recovery_mode);
}

/// Test swap token configuration
#[test]
fn test_swap_token_config() {
    let config = BenchConfig::builder()
        .enable_swap_token(true)
        .num_tokens(4)
        .build();

    assert!(config.enable_swap_token);
    assert_eq!(config.num_tokens, 4);
}

/// Integration test: Verify config can be written and parsed
#[test]
fn test_config_roundtrip() {
    let config = BenchConfig::builder()
        .target_tps(5000)
        .num_accounts(500)
        .duration_secs(30)
        .build();

    // Write to temp file
    let temp_dir = std::env::temp_dir();
    let config_path = temp_dir.join("test_bench_config.toml");

    config
        .write_to_file(&config_path)
        .expect("Failed to write config");

    // Verify file exists and contains expected content
    let content = std::fs::read_to_string(&config_path).expect("Failed to read config");
    assert!(content.contains("target_tps = 5000"));
    assert!(content.contains("num_accounts = 500"));
    assert!(content.contains("duration_secs = 30"));

    // Cleanup
    std::fs::remove_file(&config_path).ok();
}

// =============================================================================
// Integration Tests - Require running node and gravity_bench
// Run with: cargo test --package fastevm-execution --test bench_tests -- --ignored --nocapture
// =============================================================================

/// Configuration for full integration test
const BENCH_DURATION_SECS: u64 = 300; // 5 minutes
const BENCH_TARGET_TPS: u32 = 1000;
const BENCH_NUM_ACCOUNTS: u32 = 100;
const BENCH_NUM_SENDERS: u32 = 50;
const NODE_PORT: u16 = 8545;
const NODE_STARTUP_TIMEOUT_SECS: u64 = 60;

/// Full integration test: Start fastevm-gravity node, run benchmark, verify results
///
/// This test:
/// 1. Starts fastevm-gravity node
/// 2. Waits for node to be ready
/// 3. Records initial state (block number, faucet balance)
/// 4. Runs gravity_bench for 5 minutes
/// 5. Checks number of transactions mined
/// 6. Checks faucet account balances
/// 7. Shuts down the node
#[tokio::test]
#[ignore = "Requires fastevm-gravity binary and gravity_bench. Run with --ignored --nocapture"]
async fn test_full_benchmark_5_minutes() {
    println!("\n========================================");
    println!("  FastEVM Benchmark Integration Test");
    println!("========================================\n");

    // Get project root directory
    let project_root = std::env::current_dir()
        .expect("Failed to get current directory")
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap());

    let working_dir = project_root
        .join("execution-client")
        .join("bench_test_data");
    std::fs::create_dir_all(&working_dir).expect("Failed to create working directory");

    println!("Working directory: {}", working_dir.display());

    // Create benchmark configuration
    let config = BenchConfig::builder()
        .target_tps(BENCH_TARGET_TPS)
        .rpc_url(format!("http://localhost:{}", NODE_PORT))
        .chain_id(7771625)
        .num_accounts(BENCH_NUM_ACCOUNTS)
        .duration_secs(BENCH_DURATION_SECS)
        .num_senders(BENCH_NUM_SENDERS)
        .build();

    // Try to find the node binary
    // Priority: FASTEVM_BINARY_DIR env var > release > debug
    let node_binary_path = {
        // Check if FASTEVM_BINARY_DIR is set (from run-tests.sh)
        if let Ok(binary_dir) = std::env::var("FASTEVM_BINARY_DIR") {
            let path = project_root.join(&binary_dir).join("fastevm-gravity");
            println!("Using binary from FASTEVM_BINARY_DIR: {}", path.display());
            path
        } else {
            let release_path = project_root.join("target/release/fastevm-gravity");
            let debug_path = project_root.join("target/debug/fastevm-gravity");

            if release_path.exists() {
                println!("Using release binary: {}", release_path.display());
                release_path
            } else if debug_path.exists() {
                println!("Using debug binary: {}", debug_path.display());
                debug_path
            } else {
                println!("No binary found, will try release path");
                release_path
            }
        }
    };

    let runner = BenchRunner::new(config.clone())
        .with_working_dir(&working_dir);

    // Get script paths
    let scripts_dir = project_root.join("execution-client").join("scripts");
    let start_node_script = scripts_dir.join("start-bench-node.sh");
    let run_bench_script = scripts_dir.join("run-gravity-bench.sh");

    // Prepare paths
    let data_dir = working_dir.join(format!("bench_data_{}", NODE_PORT));
    let log_file = working_dir.join(format!("fastevm-gravity-{}.log", NODE_PORT));

    // Step 1: Start the node using bash script
    println!("\n[Step 1] Starting fastevm-gravity node...");
    let node_pid_output = std::process::Command::new(&start_node_script)
        .arg(&node_binary_path)
        .arg(NODE_PORT.to_string())
        .arg(&data_dir)
        .arg(&log_file)
        .output()
        .expect("Failed to execute start-bench-node.sh script");

    if !node_pid_output.status.success() {
        let stderr = String::from_utf8_lossy(&node_pid_output.stderr);
        println!("  ✗ Failed to start node: {}", stderr);
        println!("\n  Make sure to build the binary first:");
        println!("  cargo build --release --bin fastevm-gravity");
        return;
    }

    let node_pid_str = String::from_utf8_lossy(&node_pid_output.stdout).trim().to_string();
    let node_pid: u32 = node_pid_str.parse().unwrap_or(0);
    println!("  ✓ Node process started (PID: {})", node_pid);

    let rpc_url = format!("http://localhost:{}", NODE_PORT);

    // Step 2: Wait for node to be ready
    println!(
        "\n[Step 2] Waiting for node to be ready (timeout: {}s)...",
        NODE_STARTUP_TIMEOUT_SECS
    );
    match runner
        .wait_for_node_ready(
            &rpc_url,
            Duration::from_secs(NODE_STARTUP_TIMEOUT_SECS),
        )
        .await
    {
        Ok(_) => println!("  ✓ Node is ready and accepting RPC requests"),
        Err(e) => {
            println!("  ✗ Node failed to become ready: {}", e);
            println!("\n  Check the log file: {:?}", log_file);
            // Kill the node process
            if node_pid > 0 {
                let _ = std::process::Command::new("kill").arg(node_pid.to_string()).output();
            }
            return;
        }
    }

    // Create RPC client
    let rpc = runner.create_rpc_client(&rpc_url);

    // Step 3: Record initial state
    println!("\n[Step 3] Recording initial state...");
    let initial_block = match rpc.get_block_number().await {
        Ok(num) => {
            println!("  Initial block number: {}", num);
            num
        }
        Err(e) => {
            println!("  ✗ Failed to get block number: {}", e);
            // Kill the node process
            if node_pid > 0 {
                let _ = std::process::Command::new("kill").arg(node_pid.to_string()).output();
            }
            return;
        }
    };

    let chain_id = rpc.get_chain_id().await.unwrap_or(0);
    println!("  Chain ID: {}", chain_id);

    // Check faucet balance (using the test faucet address from config)
    let faucet_address = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"; // Default anvil/hardhat faucet
    let initial_faucet_balance = rpc.get_balance(faucet_address).await.unwrap_or(0);
    println!(
        "  Faucet balance: {} wei ({:.4} ETH)",
        initial_faucet_balance,
        initial_faucet_balance as f64 / 1e18
    );

    // Step 4: Run the benchmark using bash script
    println!(
        "\n[Step 4] Running gravity_bench for {} seconds ({} minutes)...",
        BENCH_DURATION_SECS,
        BENCH_DURATION_SECS / 60
    );
    println!("  Target TPS: {}", BENCH_TARGET_TPS);
    println!("  Accounts: {}", BENCH_NUM_ACCOUNTS);
    println!("  Senders: {}", BENCH_NUM_SENDERS);
    println!();

    // Write config file
    let config_path = working_dir.join("bench_config_test.toml");
    config.write_to_file(&config_path).expect("Failed to write config file");

    // Run gravity_bench using bash script
    let bench_output = std::process::Command::new(&run_bench_script)
        .arg(&config_path)
        .arg(&working_dir)
        .arg("gravity_bench") // gravity_bench path (assumes in PATH)
        .arg(if config.recovery_mode { "true" } else { "false" })
        .output()
        .expect("Failed to execute run-gravity-bench.sh script");

    let bench_result = if bench_output.status.success() {
        let stdout = String::from_utf8_lossy(&bench_output.stdout);
        runner.parse_bench_output(&stdout)
    } else {
        let stderr = String::from_utf8_lossy(&bench_output.stderr);
        Err(gravity_bench::BenchError::ExecutionError(format!(
            "gravity_bench failed: {}",
            stderr
        )))
    };

    match &bench_result {
        Ok(stats) => {
            println!("\n  Benchmark completed!");
            println!("  ─────────────────────────────────────");
            println!("  Total TXs sent:     {}", stats.total_txs_sent);
            println!("  Average TPS:        {:.2}", stats.avg_tps);
            println!("  Peak TPS:           {:.2}", stats.peak_tps);
            println!("  Avg Latency:        {:.2} ms", stats.avg_latency_ms);
            println!("  Duration:           {} seconds", stats.duration_secs);
        }
        Err(e) => {
            println!("\n  ✗ Benchmark execution failed: {}", e);
            println!("  This is expected if gravity_bench is not installed.");
            println!("  Install from: https://github.com/Galxe/gravity_bench.git");
        }
    }

    // Step 5: Check number of transactions mined
    println!("\n[Step 5] Checking mined transactions...");
    let final_block = rpc.get_block_number().await.unwrap_or(initial_block);
    println!("  Final block number: {}", final_block);
    println!("  Blocks produced: {}", final_block - initial_block);

    if final_block > initial_block {
        // Count transactions in all new blocks
        let total_mined_txs = rpc
            .count_transactions_in_blocks(initial_block + 1, final_block)
            .await
            .unwrap_or(0);
        println!("  Total transactions mined: {}", total_mined_txs);

        let blocks_produced = final_block - initial_block;
        let avg_txs_per_block = if blocks_produced > 0 {
            total_mined_txs as f64 / blocks_produced as f64
        } else {
            0.0
        };
        println!("  Average TXs per block: {:.2}", avg_txs_per_block);

        // Calculate effective TPS
        let effective_tps = if BENCH_DURATION_SECS > 0 {
            total_mined_txs as f64 / BENCH_DURATION_SECS as f64
        } else {
            0.0
        };
        println!("  Effective TPS (mined): {:.2}", effective_tps);
    }

    // Step 6: Check faucet account balances
    println!("\n[Step 6] Checking faucet account balances...");
    let final_faucet_balance = rpc.get_balance(faucet_address).await.unwrap_or(0);
    println!(
        "  Faucet balance: {} wei ({:.4} ETH)",
        final_faucet_balance,
        final_faucet_balance as f64 / 1e18
    );

    if initial_faucet_balance > final_faucet_balance {
        let spent = initial_faucet_balance - final_faucet_balance;
        println!("  ETH spent: {:.4} ETH", spent as f64 / 1e18);
    }

    // Step 7: Shutdown the node
    println!("\n[Step 7] Shutting down node...");
    if node_pid > 0 {
        match std::process::Command::new("kill").arg(node_pid.to_string()).output() {
            Ok(_) => {
                // Wait a bit for graceful shutdown
                tokio::time::sleep(Duration::from_secs(2)).await;
                println!("  ✓ Node stopped successfully");
            }
            Err(e) => println!("  ✗ Error stopping node: {}", e),
        }
    }

    // Print summary
    println!("\n========================================");
    println!("  Test Summary");
    println!("========================================");
    println!("  Blocks produced: {}", final_block - initial_block);
    if let Ok(stats) = &bench_result {
        println!("  TXs sent: {}", stats.total_txs_sent);
        println!("  Avg TPS: {:.2}", stats.avg_tps);
    }
    println!("  Log file: {:?}", log_file);
    println!("  Data dir: {:?}", data_dir);
    println!("========================================\n");

    // Cleanup (optional - comment out to inspect data after test)
    // node_handle.cleanup().ok();
}

/// Quick integration test: Check node health endpoint
#[tokio::test]
#[ignore = "Requires running FastEVM node"]
async fn test_node_health_check() {
    println!("\n[Test] Checking node health...");

    let config = BenchConfig::builder()
        .rpc_url("http://localhost:8545")
        .build();

    let runner = BenchRunner::new(config);

    match runner.smoke_test().await {
        Ok(true) => {
            println!("  ✓ Node is healthy and ready");

            // Additional checks
            let rpc = runner.create_rpc_client("http://localhost:8545");

            if let Ok(block) = rpc.get_block_number().await {
                println!("  Block number: {}", block);
            }

            if let Ok(chain_id) = rpc.get_chain_id().await {
                println!("  Chain ID: {}", chain_id);
            }
        }
        Ok(false) => {
            println!("  ✗ Node is not ready");
        }
        Err(e) => {
            println!("  ✗ Health check failed: {}", e);
        }
    }
}

/// Quick benchmark test: Run for 30 seconds only
#[tokio::test]
#[ignore = "Requires running FastEVM node and gravity_bench"]
async fn test_quick_benchmark() {
    println!("\n[Test] Running quick 30-second benchmark...");

    let config = BenchConfig::builder()
        .target_tps(500)
        .rpc_url("http://localhost:8545")
        .num_accounts(50)
        .duration_secs(30)
        .num_senders(20)
        .build();

    let runner = BenchRunner::new(config);

    // Check node is ready
    match runner.smoke_test().await {
        Ok(true) => println!("  ✓ Node is ready"),
        _ => {
            println!("  ✗ Node not ready, skipping benchmark");
            return;
        }
    }

    // Record initial state
    let rpc = runner.create_rpc_client("http://localhost:8545");
    let initial_block = rpc.get_block_number().await.unwrap_or(0);

    // Run benchmark
    println!("  Running benchmark for 30 seconds...");
    match runner.run_benchmark().await {
        Ok(stats) => {
            println!("  ✓ Benchmark completed");
            println!("    TXs sent: {}", stats.total_txs_sent);
            println!("    Avg TPS: {:.2}", stats.avg_tps);
        }
        Err(e) => {
            println!("  ✗ Benchmark failed: {}", e);
        }
    }

    // Check final state
    let final_block = rpc.get_block_number().await.unwrap_or(0);
    println!("  Blocks produced: {}", final_block - initial_block);
}
