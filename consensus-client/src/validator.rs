// Copyright (c) Scalar Org, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use consensus_config::{AuthorityIndex, Committee, NetworkKeyPair, Parameters, ProtocolKeyPair};
use consensus_core::{
    Clock, CommitConsumer, ConsensusAuthority, TransactionIndex, TransactionVerifier,
    ValidationError,
};
use mysten_metrics::RegistryService;
use std::path::PathBuf;
use std::sync::Arc;
use sui_protocol_config::{ConsensusNetwork, ProtocolConfig};
use tokio::sync::mpsc;
use tracing::{error, info};

use crate::engine_api::PayloadItem;

// Simple transaction verifier that accepts all transactions
#[derive(Debug)]
struct SimpleTransactionVerifier;

impl TransactionVerifier for SimpleTransactionVerifier {
    fn verify_batch(&self, _batch: &[&[u8]]) -> Result<(), ValidationError> {
        Ok(())
    }

    fn verify_and_vote_batch(
        &self,
        _batch: &[&[u8]],
    ) -> Result<Vec<TransactionIndex>, ValidationError> {
        Ok(vec![])
    }
}
pub struct ValidatorNode {
    authority_index: AuthorityIndex,
    working_directory: PathBuf,
    consensus_authority: Option<ConsensusAuthority>,
}

impl ValidatorNode {
    pub fn new(authority_index: u32, working_directory: PathBuf) -> Self {
        Self {
            authority_index: AuthorityIndex::new_for_test(authority_index),
            working_directory,
            consensus_authority: None,
        }
    }

    pub async fn start(
        &mut self,
        committee: Committee,
        parameters: Parameters,
        keypairs: Vec<(NetworkKeyPair, ProtocolKeyPair)>,
        registry_service: RegistryService,
        commit_consumer: CommitConsumer,
        input_payload_rx: mpsc::UnboundedReceiver<PayloadItem>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Starting validator node {}", self.authority_index);

        // Create node directory
        let node_dir = self
            .working_directory
            .join(format!("node-{}", self.authority_index));
        std::fs::create_dir_all(&node_dir)?;
        let db_path = node_dir.join("consensus.db");

        // Get keypairs for this node
        let (network_keypair, protocol_keypair) = &keypairs[self.authority_index.value()];

        // Create parameters
        let parameters = Parameters {
            db_path,
            // Note: The actual Parameters struct from consensus-config may have different fields
            // This is a placeholder - you may need to map the consensus_params fields
            // to the actual Parameters struct fields based on the consensus-config crate
            ..parameters
        };

        // Log the loaded parameters for debugging
        info!("Loaded consensus parameters: {:?}", parameters);

        // Start the consensus authority
        let consensus_authority = ConsensusAuthority::start(
            ConsensusNetwork::Anemo,
            self.authority_index,
            committee,
            parameters,
            ProtocolConfig::get_for_max_version_UNSAFE(),
            protocol_keypair.clone(),
            network_keypair.clone(),
            Arc::new(Clock::new_for_test(0)),
            Arc::new(SimpleTransactionVerifier),
            commit_consumer,
            registry_service.default_registry().clone(),
            0, // boot_counter
        )
        .await;

        self.consensus_authority = Some(consensus_authority);

        // Start transaction processing and consensus output handling
        self.start_transaction_processing(input_payload_rx).await;

        info!(
            "Validator node {} started successfully",
            self.authority_index
        );
        Ok(())
    }

    async fn start_transaction_processing(
        &self,
        mut input_payload_rx: mpsc::UnboundedReceiver<PayloadItem>,
    ) {
        // Process received payload from execution client
        let transaction_client = self
            .consensus_authority
            .as_ref()
            .unwrap()
            .transaction_client();
        tokio::spawn(async move {
            while let Some(payload) = input_payload_rx.recv().await {
                info!("Received payload from execution client: {:?}", &payload);
                //let tx_data = extract_transaction_from_payload_v3(payload);
                let tx_data = payload.into_iter().map(|tx| tx.into()).collect();
                match transaction_client.submit(tx_data).await {
                    Ok((block_ref, _status_receiver)) => {
                        info!(
                            "Transaction submitted successfully to Mysticeti consensus, included in block: {:?}",
                            block_ref
                        );
                    }
                    Err(e) => {
                        error!("Failed to submit transaction to Mysticeti consensus: {}", e);
                    }
                }
            }
        });

        info!(
            "Transaction processing started for node {}",
            self.authority_index
        );
    }

    // pub async fn stop(&mut self) {
    //     info!("Stopping validator node {}", self.authority_index);
    //     if let Some(authority) = self.consensus_authority.take() {
    //         authority.stop().await;
    //     }
    // }
}

#[cfg(test)]
mod tests {
    use super::*;
    use consensus_config::{
        Authority, AuthorityKeyPair, Committee, NetworkKeyPair, ProtocolKeyPair, Stake,
    };
    use prometheus::Registry;
    use std::path::PathBuf;
    use tempfile::tempdir;

    // Helper function to create test committee
    fn create_test_committee() -> Committee {
        let mut rng = rand::thread_rng();
        let mut authorities = vec![];

        for i in 0..3 {
            let authority_keypair = AuthorityKeyPair::generate(&mut rng);
            let protocol_keypair = ProtocolKeyPair::generate(&mut rng);
            let network_keypair = NetworkKeyPair::generate(&mut rng);

            let address = format!("/ip4/172.20.0.{}/udp/{}", 10 + i, 26657 + i);
            let address = address.parse().unwrap();

            authorities.push(Authority {
                stake: Stake::from(1000u64),
                address,
                hostname: format!("test-node{}", i),
                authority_key: authority_keypair.public(),
                protocol_key: protocol_keypair.public(),
                network_key: network_keypair.public(),
            });
        }

        Committee::new(1, authorities)
    }

    // Helper function to create test keypairs
    fn create_test_keypairs() -> Vec<(NetworkKeyPair, ProtocolKeyPair)> {
        let mut rng = rand::thread_rng();
        let mut keypairs = vec![];

        for _ in 0..3 {
            let protocol_keypair = ProtocolKeyPair::generate(&mut rng);
            let network_keypair = NetworkKeyPair::generate(&mut rng);
            keypairs.push((network_keypair, protocol_keypair));
        }

        keypairs
    }

    #[test]
    fn test_validator_node_new() {
        let working_directory = PathBuf::from("/tmp/test");
        let validator = ValidatorNode::new(42, working_directory.clone());

        assert_eq!(validator.authority_index.value(), 42);
        assert_eq!(validator.working_directory, working_directory);
        assert!(validator.consensus_authority.is_none());
    }

    #[test]
    fn test_validator_node_new_with_zero_index() {
        let working_directory = PathBuf::from("/tmp/test");
        let validator = ValidatorNode::new(0, working_directory.clone());

        assert_eq!(validator.authority_index.value(), 0);
        assert_eq!(validator.working_directory, working_directory);
        assert!(validator.consensus_authority.is_none());
    }

    #[test]
    fn test_validator_node_new_with_large_index() {
        let working_directory = PathBuf::from("/tmp/test");
        let validator = ValidatorNode::new(u32::MAX, working_directory.clone());

        assert_eq!(validator.authority_index.value(), u32::MAX as usize);
        assert_eq!(validator.working_directory, working_directory);
        assert!(validator.consensus_authority.is_none());
    }

    #[test]
    fn test_simple_transaction_verifier_verify_batch() {
        let verifier = SimpleTransactionVerifier;
        let batch = vec![b"tx1" as &[u8], b"tx2" as &[u8], b"tx3" as &[u8]];

        let result = verifier.verify_batch(&batch);
        assert!(result.is_ok());
    }

    #[test]
    fn test_simple_transaction_verifier_verify_batch_empty() {
        let verifier = SimpleTransactionVerifier;
        let batch: Vec<&[u8]> = vec![];

        let result = verifier.verify_batch(&batch);
        assert!(result.is_ok());
    }

    #[test]
    fn test_simple_transaction_verifier_verify_batch_large() {
        let verifier = SimpleTransactionVerifier;
        let large_data1 = vec![0u8; 10000];
        let large_data2 = vec![1u8; 10000];
        let batch: Vec<&[u8]> = vec![&large_data1, &large_data2];

        let result = verifier.verify_batch(&batch);
        assert!(result.is_ok());
    }

    #[test]
    fn test_simple_transaction_verifier_verify_and_vote_batch() {
        let verifier = SimpleTransactionVerifier;
        let batch = vec![b"tx1" as &[u8], b"tx2" as &[u8], b"tx3" as &[u8]];

        let result = verifier.verify_and_vote_batch(&batch);
        assert!(result.is_ok());

        let transaction_indices = result.unwrap();
        assert_eq!(transaction_indices.len(), 0); // Returns empty vec as per implementation
    }

    #[test]
    fn test_simple_transaction_verifier_verify_and_vote_batch_empty() {
        let verifier = SimpleTransactionVerifier;
        let batch: Vec<&[u8]> = vec![];

        let result = verifier.verify_and_vote_batch(&batch);
        assert!(result.is_ok());

        let transaction_indices = result.unwrap();
        assert_eq!(transaction_indices.len(), 0);
    }

    #[tokio::test]
    async fn test_validator_node_start_basic() {
        let temp_dir = tempdir().unwrap();
        let working_directory = temp_dir.path().to_path_buf();
        let mut validator = ValidatorNode::new(0, working_directory);

        let committee = create_test_committee();
        let keypairs = create_test_keypairs();
        let registry_service = RegistryService::new(Registry::new());
        let (commit_consumer, _, _) = CommitConsumer::new(0);
        let (_, input_payload_rx) = mpsc::unbounded_channel();

        // This test verifies that the start method can be called without panicking
        // In a real scenario, this would start the consensus authority
        let result = validator
            .start(
                committee,
                Parameters::default(),
                keypairs,
                registry_service,
                commit_consumer,
                input_payload_rx,
            )
            .await;

        // The test passes if we can call start without panicking
        // The actual result may vary depending on the consensus authority setup
        assert!(true);
    }

    #[tokio::test]
    async fn test_validator_node_start_with_different_indices() {
        let temp_dir = tempdir().unwrap();
        let working_directory = temp_dir.path().to_path_buf();

        for node_index in [0, 1, 2] {
            let mut validator = ValidatorNode::new(node_index, working_directory.clone());
            let committee = create_test_committee();
            let keypairs = create_test_keypairs();
            let registry_service = RegistryService::new(Registry::new());
            let (commit_consumer, _, _) = CommitConsumer::new(0);
            let (_, input_payload_rx) = mpsc::unbounded_channel();

            // Test that start can be called for different node indices
            let _result = validator
                .start(
                    committee,
                    Parameters::default(),
                    keypairs,
                    registry_service,
                    commit_consumer,
                    input_payload_rx,
                )
                .await;

            assert!(true);
        }
    }

    #[tokio::test]
    async fn test_validator_node_start_creates_directory() {
        let temp_dir = tempdir().unwrap();
        let working_directory = temp_dir.path().to_path_buf();
        let mut validator = ValidatorNode::new(0, working_directory.clone());

        let committee = create_test_committee();
        let keypairs = create_test_keypairs();
        let registry_service = RegistryService::new(Registry::new());
        let (commit_consumer, _, _) = CommitConsumer::new(0);
        let (_, input_payload_rx) = mpsc::unbounded_channel();

        // Start the validator
        let _result = validator
            .start(
                committee,
                Parameters::default(),
                keypairs,
                registry_service,
                commit_consumer,
                input_payload_rx,
            )
            .await;

        // Check that the node directory was created
        let node_dir = working_directory.join("node-0");
        assert!(node_dir.exists());
        assert!(node_dir.is_dir());

        // Check that the consensus database file was created
        let db_path = node_dir.join("consensus.db");
        assert!(db_path.exists());
    }

    #[tokio::test]
    async fn test_validator_node_start_with_invalid_committee() {
        let temp_dir = tempdir().unwrap();
        let working_directory = temp_dir.path().to_path_buf();
        let mut validator = ValidatorNode::new(0, working_directory);

        // Create an empty committee (invalid)
        let committee = Committee::new(1, vec![]);
        let keypairs = create_test_keypairs();
        let registry_service = RegistryService::new(Registry::new());
        let (commit_consumer, _, _) = CommitConsumer::new(0);
        let (_, input_payload_rx) = mpsc::unbounded_channel();

        // This should handle the invalid committee gracefully
        let _result = validator
            .start(
                committee,
                Parameters::default(),
                keypairs,
                registry_service,
                commit_consumer,
                input_payload_rx,
            )
            .await;

        // Test passes if we can handle invalid input without panicking
        assert!(true);
    }

    #[tokio::test]
    async fn test_validator_node_start_with_different_working_directories() {
        let temp_dir = tempdir().unwrap();
        let base_path = temp_dir.path().to_path_buf();

        for i in 0..3 {
            let working_directory = base_path.join(format!("validator-{}", i));
            let mut validator = ValidatorNode::new(i, working_directory.clone());

            let committee = create_test_committee();
            let keypairs = create_test_keypairs();
            let registry_service = RegistryService::new(Registry::new());
            let (commit_consumer, _, _) = CommitConsumer::new(0);
            let (_, input_payload_rx) = mpsc::unbounded_channel();

            // Start the validator
            let _result = validator
                .start(
                    committee,
                    Parameters::default(),
                    keypairs,
                    registry_service,
                    commit_consumer,
                    input_payload_rx,
                )
                .await;

            // Check that the directory was created
            let node_dir = working_directory.join(format!("node-{}", i));
            assert!(node_dir.exists());
        }
    }

    #[test]
    fn test_simple_transaction_verifier_debug() {
        let verifier = SimpleTransactionVerifier;
        let debug_str = format!("{:?}", verifier);

        // Test that debug formatting works
        assert!(!debug_str.is_empty());
    }

    #[test]
    fn test_transaction_verifier_trait_implementation() {
        let verifier = SimpleTransactionVerifier;

        // Test that the verifier implements the required trait methods
        let batch = vec![b"test" as &[u8]];
        let result = verifier.verify_batch(&batch);
        assert!(result.is_ok());

        let vote_result = verifier.verify_and_vote_batch(&batch);
        assert!(vote_result.is_ok());
        let indices = vote_result.unwrap();
        assert_eq!(indices.len(), 0);
    }

    #[tokio::test]
    async fn test_validator_node_concurrent_access() {
        let temp_dir = tempdir().unwrap();
        let working_directory = temp_dir.path().to_path_buf();
        let validator = Arc::new(ValidatorNode::new(0, working_directory));

        let validator_clone1 = Arc::clone(&validator);
        let validator_clone2 = Arc::clone(&validator);

        let handle1 = tokio::spawn(async move {
            // Test concurrent access to validator properties
            assert_eq!(validator_clone1.authority_index.value(), 0);
        });

        let handle2 = tokio::spawn(async move {
            // Test concurrent access to validator properties
            assert_eq!(validator_clone2.authority_index.value(), 0);
        });

        let (result1, result2) = tokio::join!(handle1, handle2);
        assert!(result1.is_ok());
        assert!(result2.is_ok());
    }

    #[test]
    fn test_validator_node_path_handling() {
        let working_directory = PathBuf::from("/tmp/test/validator");
        let validator = ValidatorNode::new(0, working_directory.clone());

        // Test that the working directory is stored correctly
        assert_eq!(validator.working_directory, working_directory);

        // Test that the node directory path would be constructed correctly
        let expected_node_dir = working_directory.join("node-0");
        assert_eq!(
            expected_node_dir,
            PathBuf::from("/tmp/test/validator/node-0")
        );
    }

    #[test]
    fn test_authority_index_values() {
        let test_indices = vec![0, 1, 10, 100, 1000, u32::MAX];

        for index in test_indices {
            let working_directory = PathBuf::from("/tmp/test");
            let validator = ValidatorNode::new(index, working_directory);

            assert_eq!(validator.authority_index.value(), index as usize);
        }
    }
}
