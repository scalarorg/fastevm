/// Our custom cli args extension that adds one flag to reth default CLI.
#[derive(Debug, Clone, Copy, Default, clap::Args)]
pub(crate) struct CliMysticetiArgs {
    /// CLI flag to enable the txpool extension namespace
    #[arg(long)]
    pub enable_tx_subscription: bool,
    /// Number of transactions to send in a batch
    #[arg(long)]
    pub committed_subdags_per_block: usize,
    /// Build interval in milliseconds
    #[arg(long)]
    pub block_build_interval_ms: u64,
    /// Maximum number of accounts to reload during pool maintenance
    #[arg(long, default_value = "500")]
    pub max_reload_accounts: u64,
}
