// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use alloy_rpc_types_engine::ExecutionPayloadFieldV2;
use anyhow::Result;
use consensus_config::{AuthorityIndex, NetworkKeyPair, Parameters, ProtocolKeyPair};
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
// Simple transaction verifier that accepts all transactions
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
        committee: consensus_config::Committee,
        keypairs: Vec<(NetworkKeyPair, ProtocolKeyPair)>,
        registry_service: RegistryService,
        commit_consumer: CommitConsumer,
        input_payload_rx: mpsc::UnboundedReceiver<ExecutionPayloadFieldV2>,
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
            ..Default::default()
        };

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
        mut input_payload_rx: mpsc::UnboundedReceiver<ExecutionPayloadFieldV2>,
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
                let tx_data = extract_transaction(payload);
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

    pub async fn stop(&mut self) {
        info!("Stopping validator node {}", self.authority_index);
        if let Some(authority) = self.consensus_authority.take() {
            authority.stop().await;
        }
    }
}

fn extract_transaction(payload: ExecutionPayloadFieldV2) -> Vec<Vec<u8>> {
    let payload_v1 = match payload {
        ExecutionPayloadFieldV2::V1(execution_payload_v1) => execution_payload_v1,
        ExecutionPayloadFieldV2::V2(execution_payload_v2) => execution_payload_v2.payload_inner,
    };
    let tx_data = payload_v1
        .transactions
        .into_iter()
        .map(|tx| tx.0.into())
        .collect();
    tx_data
}
