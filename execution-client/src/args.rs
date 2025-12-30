/// Our custom cli args extension that adds one flag to reth default CLI.
#[derive(Debug, Clone, Copy, Default, clap::Args)]
pub struct CliMysticetiArgs {
    /// Number of transactions to send in a batch
    #[arg(long)]
    pub committed_subdags_per_block: usize,
    /// Build interval in milliseconds
    #[arg(long)]
    pub block_interval_ms: u64,
    /// Maximum number of accounts to reload during pool maintenance
    #[arg(long, default_value = "500")]
    pub max_reload_accounts: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    /// Wrapper struct for testing CLI arguments
    #[derive(Debug, Parser)]
    struct TestCliWrapper {
        #[command(flatten)]
        args: CliMysticetiArgs,
    }

    #[test]
    fn test_cli_mysticeti_args_default() {
        let args = CliMysticetiArgs::default();

        assert!(!args.enable_tx_subscription);
        assert_eq!(args.committed_subdags_per_block, 0);
        assert_eq!(args.block_interval_ms, 0);
        assert_eq!(args.max_reload_accounts, 0); // Default derive gives 0, not 500
    }

    #[test]
    fn test_cli_mysticeti_args_parsing_basic() {
        let args = TestCliWrapper::parse_from([
            "test",
            "--committed-subdags-per-block",
            "10",
            "--block-interval-ms",
            "100",
        ]);

        assert!(!args.args.enable_tx_subscription);
        assert_eq!(args.args.committed_subdags_per_block, 10);
        assert_eq!(args.args.block_interval_ms, 100);
    }

    #[test]
    fn test_cli_mysticeti_args_parsing_with_tx_subscription() {
        let args = TestCliWrapper::parse_from([
            "test",
            "--committed-subdags-per-block",
            "5",
            "--block-interval-ms",
            "50",
        ]);

        assert!(args.args.enable_tx_subscription);
        assert_eq!(args.args.committed_subdags_per_block, 5);
        assert_eq!(args.args.block_interval_ms, 50);
    }

    #[test]
    fn test_cli_mysticeti_args_parsing_with_max_reload_accounts() {
        let args = TestCliWrapper::parse_from([
            "test",
            "--committed-subdags-per-block",
            "1",
            "--block-interval-ms",
            "10",
            "--max-reload-accounts",
            "1000",
        ]);

        assert_eq!(args.args.max_reload_accounts, 1000);
    }

    #[test]
    fn test_cli_mysticeti_args_default_max_reload_accounts() {
        // When parsed with clap, the default value should be 500
        let args = TestCliWrapper::parse_from([
            "test",
            "--committed-subdags-per-block",
            "1",
            "--block-interval-ms",
            "10",
        ]);

        assert_eq!(args.args.max_reload_accounts, 500);
    }

    #[test]
    fn test_cli_mysticeti_args_clone() {
        let args = CliMysticetiArgs {
            committed_subdags_per_block: 10,
            block_interval_ms: 100,
            max_reload_accounts: 500,
        };

        let cloned = args.clone();

        assert_eq!(args.enable_tx_subscription, cloned.enable_tx_subscription);
        assert_eq!(
            args.committed_subdags_per_block,
            cloned.committed_subdags_per_block
        );
        assert_eq!(args.block_interval_ms, cloned.block_interval_ms);
        assert_eq!(args.max_reload_accounts, cloned.max_reload_accounts);
    }

    #[test]
    fn test_cli_mysticeti_args_debug() {
        let args = CliMysticetiArgs {
            committed_subdags_per_block: 10,
            block_interval_ms: 100,
            max_reload_accounts: 500,
        };

        let debug_str = format!("{:?}", args);

        assert!(debug_str.contains("committed_subdags_per_block: 10"));
        assert!(debug_str.contains("block_interval_ms: 100"));
        assert!(debug_str.contains("max_reload_accounts: 500"));
    }
}
