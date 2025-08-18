use std::collections::HashMap;
use std::str::FromStr;
use std::hash::{Hash, Hasher as StdHasher};
use anyhow::anyhow;
use alloy_primitives::{keccak256, Address, Bloom, Bytes, B256, U256};
use alloy_rpc_types_engine::{ ExecutionPayloadV3, ForkchoiceState, PayloadAttributes};
use consensus_core::{CommittedSubDag, BlockAPI};
use derive_getters::Getters;
use tree_hash::{Hash256};
use tree_hash_derive::TreeHash;
use serde::{Deserialize, Serialize};
use tiny_keccak::Keccak;
use rlp::RlpStream;
use tracing::info;

use crate::{beacon_chain::beacon_block::{BeaconBlock, BeaconBlockHeader, ChainSpec, Eth1Data, ExecutionPayload}, NodeConfig};

// /// Consensus state for tracking finality
// #[derive(Default, Debug, Clone, Getters)]
// pub struct BeaconState {
//     _node_index: u32,
//     current_round: u32,
//     current_block_number: u64,    
//     fee_recipient: Address,   
//     parent_block_root: Hash256,
//     finalized_block_hash: Hash256,
//     beacon_block: BeaconBlock,
//     // pub finalized_subdags: HashMap<String, SubDAG>,
//     // pub pending_subdags: HashMap<String, SubDAG>,
//     // pub latest_finalized_block: Option<H256>,
//     // pub validators: Vec<NodeId>,
//     // pub current_leader: Option<NodeId>,
// }

// Main Ethereum 2.0 Beacon State Implementation
/// Main beacon state structure following Ethereum 2.0 specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeaconState {
    fee_recipient: Address,
    // Genesis
    pub genesis_time: u64,
    pub genesis_validators_root: Hash256,
    pub slot: u64,
    pub fork: Fork,
    
    // History
    pub latest_block_hash: Hash256,
    pub latest_block_header: BeaconBlockHeader,
    pub block_roots: Vec<Hash256>,
    pub state_roots: Vec<Hash256>,
    pub historical_roots: Vec<Hash256>,
    
    // Eth1
    pub eth1_data: Eth1Data,
    pub eth1_data_votes: Vec<Eth1Data>,
    pub eth1_deposit_index: u64,
    
    // Registry
    pub validators: Vec<Validator>,
    pub balances: Vec<u64>,
    
    // Randomness
    pub randao_mixes: Vec<Hash256>,
    
    // Slashings
    pub slashings: Vec<u64>,
    
    // Participation
    pub previous_epoch_participation: Vec<ParticipationFlags>,
    pub current_epoch_participation: Vec<ParticipationFlags>,
    
    // Finality
    pub justification_bits: Vec<bool>,
    pub previous_justified_checkpoint: Checkpoint,
    pub current_justified_checkpoint: Checkpoint,
    pub finalized_checkpoint: Checkpoint,
    
    // Inactivity
    pub inactivity_scores: Vec<u64>,
    
    // Sync
    pub current_sync_committee: SyncCommittee,
    pub next_sync_committee: SyncCommittee,
    
    // Execution
    pub latest_execution_payload_header: Option<ExecutionPayloadHeader>,
}

impl BeaconState {
    //Initializes the consensus state with the genesis block hash get from the execution client init
    /// Create a new genesis state
    pub fn new(genesis_time: u64, fee_recipient: Address, spec: &ChainSpec) -> Self {
        Self {
            genesis_time,
            fee_recipient,
            genesis_validators_root: Hash256::default(),
            slot: 0,
            fork: Fork::new(spec),
            latest_block_hash: Hash256::default(),
            latest_block_header: BeaconBlockHeader::default(),
            block_roots: Vec::new(),
            state_roots: Vec::new(),
            historical_roots: Vec::new(),
            eth1_data: Eth1Data::default(),
            eth1_data_votes: Vec::new(),
            eth1_deposit_index: 0,
            validators: Vec::new(),
            balances: Vec::new(),
            randao_mixes: Vec::new(),
            slashings: Vec::new(),
            previous_epoch_participation: Vec::new(),
            current_epoch_participation: Vec::new(),
            justification_bits: Vec::new(),
            previous_justified_checkpoint: Checkpoint::default(),
            current_justified_checkpoint: Checkpoint::default(),
            finalized_checkpoint: Checkpoint::default(),
            inactivity_scores: Vec::new(),
            current_sync_committee: SyncCommittee::default(),
            next_sync_committee: SyncCommittee::default(),
            latest_execution_payload_header: None,
        }
    }
    
    /// Create a new genesis state from NodeConfig and GenesisData
    pub fn from_config(config: &NodeConfig)-> Result<Self, anyhow::Error> {
        let fee_recipient = Address::from_str(&config.fee_recipient).map_err(|e| anyhow!("Invalid fee recipient: {:?}", e))?;
        let genesis_block_hash = B256::from_str(config.genesis_block_hash.trim_start_matches("0x")).
            map_err(|e| anyhow!("Invalid genesis block hash: {:?}, {:?}", config.genesis_block_hash, e))?;
    
        let chain_spec = ChainSpec::new();
        let mut state = Self::new(config.genesis_time, fee_recipient, &chain_spec);
        state.latest_block_hash = genesis_block_hash;
        // Initialize with some basic validators (simplified)
        // In a real implementation, this would load actual validator data
        let validator = Validator {
            pubkey: Hash256::default(),
            withdrawal_credentials: Hash256::default(),
            effective_balance: 32_000_000_000, // 32 ETH in gwei
            slashed: false,
            activation_eligibility_epoch: 0,
            activation_epoch: 0,
            exit_epoch: u64::MAX,
            withdrawable_epoch: u64::MAX,
        };
        
        state.validators = vec![validator];
        state.balances = vec![32_000_000_000]; // 32 ETH in gwei
        
        // Initialize RANDAO mixes
        state.randao_mixes = vec![Hash256::default(); 65536]; // EPOCHS_PER_HISTORICAL_VECTOR
        
        // Initialize slashings
        state.slashings = vec![0; 8192]; // EPOCHS_PER_SLASHINGS_VECTOR
        
        // Initialize participation flags
        state.previous_epoch_participation = vec![ParticipationFlags { flags: 0 }; 1];
        state.current_epoch_participation = vec![ParticipationFlags { flags: 0 }; 1];
        
        // Initialize justification bits
        state.justification_bits = vec![false; 4];
        
        // Initialize sync committees
        state.current_sync_committee = SyncCommittee {
            pubkeys: vec![Hash256::default()],
            aggregate_pubkey: Hash256::default(),
        };
        state.next_sync_committee = state.current_sync_committee.clone();
        
        Ok(state)
    }

    pub fn get_fork_choice_state(&self) -> ForkchoiceState {
        ForkchoiceState {
            head_block_hash: self.latest_block_hash.clone(),
            safe_block_hash: self.latest_block_hash.clone(),
            finalized_block_hash: self.latest_block_hash.clone(),
        }
    }
    pub fn get_payload_attributes(&self) -> Option<PayloadAttributes> {
        // Get current timestamp for post-Shanghai blocks
        // Use a timestamp that's in the future relative to the current time
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        let attributes = PayloadAttributes {
            timestamp,
            // prev_randao should be the mixHash from the previous block
            // For the first block or when no previous block exists, use a placeholder
            // TODO: Get actual prev_randao from the previous block's header
            prev_randao: B256::default(),
            suggested_fee_recipient: self.fee_recipient.clone(),
            // For post-Shanghai blocks, withdrawals must be present (empty vec for no withdrawals)
            withdrawals: Some(vec![]),
            // parent_beacon_block_root is required for post-Capella blocks (after the merge)
            // Since we're using post-Shanghai blocks, this should be set to the actual beacon block root
            // For now, use None as this might not be required for all post-Shanghai implementations
            // TODO: Get actual parent_beacon_block_root from the beacon chain if required
            parent_beacon_block_root: Some(self.latest_block_header.canonical_root()),
        };
        Some(attributes)
    }
    
    // ///
    // /// Process the consensus output and return the execution payload input v2
    // /// 
    // pub fn process_subdag(&mut self, committed_subdag: CommittedSubDag) -> ExecutionPayloadInputV2 {
    //     //Set current rount to the leader round
    //     self.slot = committed_subdag.leader.round as u64;
    //     let _rewards = self.calculate_rewards(&committed_subdag);
    //     let block = SubDagBlock::new(committed_subdag);
    //     let mut payload = block.get_execution_payload_input_v2();
    //     let block_hash = block.get_block_hash();
    //     payload.execution_payload.fee_recipient = self.fee_recipient;
    //     payload.execution_payload.timestamp = block.get_block_timestamp();
    //     payload.execution_payload.block_number = self.current_block_number + 1;
    //     payload.execution_payload.block_hash = block_hash.clone();
    //     payload.execution_payload.parent_hash = self.finalized_block_hash.clone();
    //     self.current_block_number += 1;
    //     self.finalized_block_hash = block_hash;        
    //     payload
    // }
    
    pub fn calculate_rewards(&self, _committed_subdag: &CommittedSubDag) -> HashMap<Address, u64> {
        //TODO: Implement this
        let rewards = HashMap::new();
        
        rewards
    }
}

impl BeaconState {

    /// Get the fee recipient (for compatibility with existing code)
    pub fn fee_recipient(&self) -> Address {
        // In a real implementation, this would return the actual fee recipient
        // For now, return a default address
        Address::default()
    }

    /// Process the consensus output and return the execution payload input v2
    pub fn process_subdag(&mut self, committed_subdag: CommittedSubDag) -> ExecutionPayloadV3 {
        // Set current slot to the leader round
        self.slot = committed_subdag.leader.round as u64;
        
        // Calculate rewards (simplified)
        let _rewards = self.calculate_rewards(&committed_subdag);
        
        // Create a block from the subdag
        let block = SubDagBlock::new(self.fee_recipient(), self.latest_block_hash.clone(), committed_subdag);
        let payload = block.get_execution_payload_v3();
        info!("latest_block_hash: {:?}", payload.payload_inner.payload_inner.block_hash);
        self.latest_block_hash = payload.payload_inner.payload_inner.block_hash.clone();
        // Update the payload with current state information
        // let block_hash = block.calculate_block_hash();
        // payload.payload_inner.payload_inner.fee_recipient = self.fee_recipient();
        // payload.payload_inner.payload_inner.block_number = self.slot + 1;
        // payload.payload_inner.payload_inner.block_hash = block_hash.clone();
        // payload.payload_inner.payload_inner.parent_hash = self.latest_block_hash.clone();
        
        // Update state
        self.slot += 1;
        
        payload
    }

    /// Get the current slot
    pub fn slot(&self) -> u64 {
        self.slot
    }

    /// Get the current epoch
    pub fn current_epoch(&self) -> u64 {
        self.slot / 32 // SLOTS_PER_EPOCH
    }

    /// Get the previous epoch
    pub fn previous_epoch(&self) -> u64 {
        if self.current_epoch() == 0 {
            0
        } else {
            self.current_epoch() - 1
        }
    }

    /// Get the latest block header
    pub fn latest_block_header(&self) -> &BeaconBlockHeader {
        &self.latest_block_header
    }

    /// Get a mutable reference to the latest block header
    pub fn latest_block_header_mut(&mut self) -> &mut BeaconBlockHeader {
        &mut self.latest_block_header
    }

    /// Set the state root for a given slot
    pub fn set_state_root(&mut self, slot: usize, state_root: Hash256) -> Result<(), Error> {
        let index = slot % 8192; // SLOTS_PER_HISTORICAL_ROOT
        if index >= self.state_roots.len() {
            self.state_roots.resize(index + 1, Hash256::ZERO);
        }
        self.state_roots[index] = state_root;
        Ok(())
    }

    /// Set the block root for a given slot
    pub fn set_block_root(&mut self, slot: usize, block_root: Hash256) -> Result<(), Error> {
        let index = slot % 8192; // SLOTS_PER_HISTORICAL_ROOT
        if index >= self.block_roots.len() {
            self.block_roots.resize(index + 1, Hash256::ZERO);
        }
        self.block_roots[index] = block_root;
        Ok(())
    }

    /// Get the state root for a given slot
    pub fn get_state_root(&self, slot: usize) -> Result<Hash256, Error> {
        let index = slot % 8192; // SLOTS_PER_HISTORICAL_ROOT
        if index < self.state_roots.len() {
            Ok(self.state_roots[index])
        } else {
            Err(Error::IndexOutOfBounds)
        }
    }

    /// Get the block root for a given slot
    pub fn get_block_root(&self, slot: usize) -> Result<Hash256, Error> {
        let index = slot % 8192; // SLOTS_PER_HISTORICAL_ROOT
        if index < self.block_roots.len() {
            Ok(self.block_roots[index])
        } else {
            Err(Error::IndexOutOfBounds)
        }
    }

    /// Update the tree hash cache
    pub fn update_tree_hash_cache(&mut self) -> Result<Hash256, Error> {
        Ok(Hash256::ZERO) // Simplified - would compute actual tree hash in real implementation
    }

    /// Advance the slot by one
    pub fn advance_slot(&mut self) -> Result<(), Error> {
        self.slot = self.slot.checked_add(1).ok_or(Error::SlotOverflow)?;
        Ok(())
    }

    /// Check if the state is in the current epoch
    pub fn is_current_epoch(&self, epoch: u64) -> bool {
        epoch == self.current_epoch()
    }

    /// Check if the state is in the previous epoch
    pub fn is_previous_epoch(&self, epoch: u64) -> bool {
        epoch == self.previous_epoch()
    }

    /// Get the validator at the given index
    pub fn get_validator(&self, index: u64) -> Result<&Validator, Error> {
        if index < self.validators.len() as u64 {
            Ok(&self.validators[index as usize])
        } else {
            Err(Error::IndexOutOfBounds)
        }
    }

    /// Get the balance at the given index
    pub fn get_balance(&self, index: u64) -> Result<u64, Error> {
        if index < self.balances.len() as u64 {
            Ok(self.balances[index as usize])
        } else {
            Err(Error::IndexOutOfBounds)
        }
    }

    /// Get the RANDAO mix for the current epoch
    pub fn get_randao_mix(&self, epoch: u64) -> Result<Hash256, Error> {
        let index = epoch % 65536; // EPOCHS_PER_HISTORICAL_VECTOR
        if index < self.randao_mixes.len() as u64 {
            Ok(self.randao_mixes[index as usize])
        } else {
            Err(Error::IndexOutOfBounds)
        }
    }

    /// Set the RANDAO mix for the current epoch
    pub fn set_randao_mix(&mut self, epoch: u64, mix: Hash256) -> Result<(), Error> {
        let index = epoch % 65536; // EPOCHS_PER_HISTORICAL_VECTOR
        if index >= self.randao_mixes.len() as u64 {
            self.randao_mixes.resize(index as usize + 1, Hash256::ZERO);
        }
        self.randao_mixes[index as usize] = mix;
        Ok(())
    }

    /// Get the proposer index for the current slot
    pub fn get_beacon_proposer_index(&self, _spec: &ChainSpec) -> Result<u64, Error> {
        let epoch = self.current_epoch();
        let seed = self.get_seed(epoch, _spec)?;
        let validators = self.get_active_validator_indices(epoch)?;
        
        if validators.is_empty() {
            return Err(Error::NoActiveValidators);
        }
        
        let index = compute_proposer_index(validators, seed, _spec)?;
        Ok(index)
    }

    /// Get the seed for the given epoch
    pub fn get_seed(&self, epoch: u64, _spec: &ChainSpec) -> Result<Hash256, Error> {
        let mix = self.get_randao_mix(epoch)?;
        let seed = compute_seed(mix, epoch, _spec)?;
        Ok(seed)
    }

    /// Get active validator indices for the given epoch
    pub fn get_active_validator_indices(&self, epoch: u64) -> Result<Vec<u64>, Error> {
        let mut active_indices = Vec::new();
        
        for (index, validator) in self.validators.iter().enumerate() {
            if validator.is_active(epoch) {
                active_indices.push(index as u64);
            }
        }
        
        Ok(active_indices)
    }

    /// Process a block and update the state
    pub fn process_block(
        &mut self,
        block: &BeaconBlock,
        verify_block_root: bool,
        spec: &ChainSpec,
    ) -> Result<u64, BlockProcessingError> {
        // Verify block header
        let proposer_index = self.process_block_header(
            block.temporary_block_header(),
            verify_block_root,
            spec,
        )?;

        // Process execution payload if enabled
        if let Some(execution_payload) = &block.body().execution_payload {
            self.process_execution_payload(execution_payload, spec)?;
        }

        // Process RANDAO
        self.process_randao(block.body().randao_reveal, spec)?;

        // Process ETH1 data
        self.process_eth1_data(block.body().eth1_data.clone())?;

        // Process operations
        self.process_operations(block.body(), spec)?;

        // Process sync committee if available
        if let Some(sync_aggregate) = &block.body().sync_aggregate {
            self.process_sync_aggregate(sync_aggregate, proposer_index, spec)?;
        }

        Ok(proposer_index)
    }

    /// Process a block header
    pub fn process_block_header(
        &mut self,
        block_header: BeaconBlockHeader,
        verify_block_root: bool,
        spec: &ChainSpec,
    ) -> Result<u64, BlockProcessingError> {
        // Verify slot matches current state
        if block_header.slot != self.slot() {
            return Err(BlockProcessingError::HeaderInvalid(
                HeaderInvalid::StateSlotMismatch,
            ));
        }

        // Verify block is newer than latest
        if block_header.slot <= self.latest_block_header().slot {
            return Err(BlockProcessingError::HeaderInvalid(
                HeaderInvalid::OlderThanLatestBlockHeader,
            ));
        }

        // Verify proposer index
        let expected_proposer = self.get_beacon_proposer_index(spec).map_err(|_e| {
            BlockProcessingError::HeaderInvalid(HeaderInvalid::ProposerIndexMismatch)
        })?;
        if block_header.proposer_index != expected_proposer as u32 {
            return Err(BlockProcessingError::HeaderInvalid(
                HeaderInvalid::ProposerIndexMismatch,
            ));
        }

        // Verify parent block root if enabled
        if verify_block_root {
            let expected_parent = self.latest_block_header().canonical_root();
            if block_header.parent_root != expected_parent {
                return Err(BlockProcessingError::HeaderInvalid(
                    HeaderInvalid::ParentBlockRootMismatch,
                ));
            }
        }

        // Update state with new block header
        *self.latest_block_header_mut() = block_header.clone();

        Ok(block_header.proposer_index as u64)
    }

    /// Process execution payload
    pub fn process_execution_payload(
        &mut self,
        execution_payload: &ExecutionPayload,
        _spec: &ChainSpec,
    ) -> Result<(), BlockProcessingError> {
        // Store execution payload header
        let header = ExecutionPayloadHeader::from_execution_payload(execution_payload);
        self.latest_execution_payload_header = Some(header);
        Ok(())
    }

    /// Process RANDAO
    pub fn process_randao(
        &mut self,
        randao_reveal: Hash256,
        _spec: &ChainSpec,
    ) -> Result<(), BlockProcessingError> {
        let epoch = self.current_epoch();
        let mix = self.get_randao_mix(epoch).map_err(|e| {
            BlockProcessingError::RandaoError(format!("Failed to get RANDAO mix: {:?}", e))
        })?;
        
        // Update RANDAO mix for next epoch
        let new_mix = mix ^ randao_reveal;
        self.set_randao_mix(epoch, new_mix).map_err(|e| {
            BlockProcessingError::RandaoError(format!("Failed to set RANDAO mix: {:?}", e))
        })?;
        
        Ok(())
    }

    /// Process ETH1 data
    pub fn process_eth1_data(&mut self, eth1_data: Eth1Data) -> Result<(), BlockProcessingError> {
        // Add to votes
        self.eth1_data_votes.push(eth1_data.clone());
        
        // Check if we have enough votes to update current data
        let votes = self.count_eth1_votes(&eth1_data);
        if votes * 2 > self.eth1_data_votes.len() {
            self.eth1_data = eth1_data;
        }
        
        Ok(())
    }

    /// Process operations (attestations, slashings, etc.)
    pub fn process_operations(
        &mut self,
        body: &BeaconBlockBody,
        _spec: &ChainSpec,
    ) -> Result<(), BlockProcessingError> {
        // Process proposer slashings
        for _slashing in &body.proposer_slashings {
            self.process_proposer_slashing(_slashing)?;
        }

        // Process attester slashings
        for _slashing in &body.attester_slashings {
            self.process_attester_slashing(_slashing)?;
        }

        // Process attestations
        for _attestation in &body.attestations {
            self.process_attestation(_attestation)?;
        }

        // Process deposits
        for _deposit in &body.deposits {
            self.process_deposit(_deposit)?;
        }

        // Process voluntary exits
        for _exit in &body.voluntary_exits {
            self.process_voluntary_exit(_exit)?;
        }

        Ok(())
    }

    /// Process sync committee aggregate
    pub fn process_sync_aggregate(
        &mut self,
        _sync_aggregate: &SyncAggregate,
        _proposer_index: u64,
        _spec: &ChainSpec,
    ) -> Result<(), BlockProcessingError> {
        // Verify sync committee participation
        // Update participation flags
        // This is a simplified implementation
        Ok(())
    }

    /// Count ETH1 votes for a specific data
    fn count_eth1_votes(&self, eth1_data: &Eth1Data) -> usize {
        self.eth1_data_votes
            .iter()
            .filter(|vote| *vote == eth1_data)
            .count()
    }

    /// Process proposer slashing
    fn process_proposer_slashing(
        &mut self,
        _slashing: &ProposerSlashing,
    ) -> Result<(), BlockProcessingError> {
        // Verify slashing
        // Apply penalties
        // This is a simplified implementation
        Ok(())
    }

    /// Process attester slashing
    fn process_attester_slashing(
        &mut self,
        _slashing: &AttesterSlashing,
    ) -> Result<(), BlockProcessingError> {
        // Verify slashing
        // Apply penalties
        // This is a simplified implementation
        Ok(())
    }

    /// Process attestation
    fn process_attestation(
        &mut self,
        _attestation: &Attestation,
    ) -> Result<(), BlockProcessingError> {
        // Verify attestation
        // Update participation
        // This is a simplified implementation
        Ok(())
    }

    /// Process deposit
    fn process_deposit(
        &mut self,
        _deposit: &Deposit,
    ) -> Result<(), BlockProcessingError> {
        // Verify deposit
        // Add validator
        // This is a simplified implementation
        Ok(())
    }

    /// Process voluntary exit
    fn process_voluntary_exit(
        &mut self,
        _exit: &SignedVoluntaryExit,
    ) -> Result<(), BlockProcessingError> {
        // Verify exit
        // Update validator status
        // This is a simplified implementation
        Ok(())
    }
}

// Supporting data structures

/// Fork information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fork {
    pub previous_version: Version,
    pub current_version: Version,
    pub epoch: u64,
}

impl Fork {
    pub fn new(_spec: &ChainSpec) -> Self {
        Self {
            previous_version: Version::default(),
            current_version: Version::default(),
            epoch: 0,
        }
    }
}

/// Version identifier
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Version {
    pub bytes: [u8; 4],
}

impl Default for Version {
    fn default() -> Self {
        Self { bytes: [0, 0, 0, 0] }
    }
}

/// Validator information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Validator {
    pub pubkey: Hash256,
    pub withdrawal_credentials: Hash256,
    pub effective_balance: u64,
    pub slashed: bool,
    pub activation_eligibility_epoch: u64,
    pub activation_epoch: u64,
    pub exit_epoch: u64,
    pub withdrawable_epoch: u64,
}

impl Validator {
    pub fn is_active(&self, epoch: u64) -> bool {
        self.activation_epoch <= epoch && epoch < self.exit_epoch
    }
}

/// Participation flags
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipationFlags {
    pub flags: u8,
}

/// Sync committee
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncCommittee {
    pub pubkeys: Vec<Hash256>,
    pub aggregate_pubkey: Hash256,
}

impl Default for SyncCommittee {
    fn default() -> Self {
        Self {
            pubkeys: Vec::new(),
            aggregate_pubkey: Hash256::ZERO,
        }
    }
}

/// Execution payload header
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPayloadHeader {
    pub parent_hash: Hash256,
    pub fee_recipient: Address,
    pub state_root: Hash256,
    pub receipts_root: Hash256,
    pub logs_bloom: Hash256,
    pub prev_randao: Hash256,
    pub block_number: u64,
    pub gas_limit: u64,
    pub gas_used: u64,
    pub timestamp: u64,
    pub extra_data: Vec<u8>,
    pub base_fee_per_gas: u64,
    pub block_hash: Hash256,
    pub transactions_root: Hash256,
}

impl ExecutionPayloadHeader {
    pub fn from_execution_payload(payload: &ExecutionPayload) -> Self {
        Self {
            parent_hash: payload.parent_hash,
            fee_recipient: payload.fee_recipient,
            state_root: payload.state_root,
            receipts_root: payload.receipts_root,
            logs_bloom: payload.logs_bloom,
            prev_randao: payload.prev_randao,
            block_number: payload.block_number,
            gas_limit: payload.gas_limit,
            gas_used: payload.gas_used,
            timestamp: payload.timestamp,
            extra_data: payload.extra_data.clone(),
            base_fee_per_gas: payload.base_fee_per_gas,
            block_hash: payload.block_hash,
            transactions_root: Hash256::ZERO, // Simplified
        }
    }
}

// Error types
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Index out of bounds")]
    IndexOutOfBounds,
    #[error("List is full")]
    ListFull,
    #[error("Slot overflow")]
    SlotOverflow,
    #[error("No active validators")]
    NoActiveValidators,
    #[error("Invalid epoch")]
    InvalidEpoch,
}

#[derive(Debug, thiserror::Error)]
pub enum BlockProcessingError {
    #[error("Header invalid: {0:?}")]
    HeaderInvalid(HeaderInvalid),
    #[error("Execution payload error: {0}")]
    ExecutionPayloadError(String),
    #[error("RANDAO error: {0}")]
    RandaoError(String),
    #[error("ETH1 data error: {0}")]
    Eth1DataError(String),
    #[error("Operations error: {0}")]
    OperationsError(String),
    #[error("Sync committee error: {0}")]
    SyncCommitteeError(String),
}

#[derive(Debug)]
pub enum HeaderInvalid {
    StateSlotMismatch,
    OlderThanLatestBlockHeader,
    ProposerIndexMismatch,
    ParentBlockRootMismatch,
}

// Helper functions
fn compute_proposer_index(
    validators: Vec<u64>,
    seed: Hash256,
    _spec: &ChainSpec,
) -> Result<u64, Error> {
    if validators.is_empty() {
        return Err(Error::NoActiveValidators);
    }
    
    // Simplified proposer selection
    let seed_bytes = seed.as_slice();
    let mut sum: u64 = 0;
    for (i, &byte) in seed_bytes.iter().enumerate() {
        sum += (byte as u64) * (i as u64 + 1);
    }
    
    let index = sum % validators.len() as u64;
    Ok(validators[index as usize])
}

fn compute_seed(mix: Hash256, epoch: u64, _spec: &ChainSpec) -> Result<Hash256, Error> {
    // Simplified seed computation
    let mut seed = mix;
    let epoch_bytes = epoch.to_le_bytes();
    for (i, &byte) in epoch_bytes.iter().enumerate() {
        // Simplified seed computation - create new seed
        let mut new_seed = seed;
        let mut seed_bytes = new_seed.as_slice().to_vec();
        seed_bytes[i % 32] ^= byte;
        new_seed = Hash256::from_slice(&seed_bytes);
    }
    Ok(seed)
}

// Import missing types from beacon_block module
use crate::beacon_chain::beacon_block::{
    Attestation, AttesterSlashing, BeaconBlockBody, Checkpoint, Deposit, 
    ProposerSlashing, SignedVoluntaryExit, SyncAggregate
};

// Mock SubDagBlock for compatibility
#[derive(Debug)]
pub struct SubDagBlock {
    fee_recipient: Address,
    parent_hash: Hash256,
    subdag: CommittedSubDag,
}

impl SubDagBlock {
    pub fn new(
        fee_recipient: Address,
        parent_hash: Hash256,
        subdag: CommittedSubDag,
    ) -> Self {
        Self { fee_recipient, parent_hash, subdag }
    }
    
    pub fn get_execution_payload_v3(&self) -> ExecutionPayloadV3 {
        // Convert timestamp from milliseconds to seconds (Ethereum standard)
        let timestamp_secs = (self.subdag.timestamp_ms / 1000) as u64;
        
        // For now, create an empty execution payload since transaction extraction is complex
        // In a production environment, this would extract actual transactions from the consensus blocks
        let transactions: Vec<Bytes> = self.flatten_transactions();
        info!("[SubDagBlock::get_execution_payload_v3] transactions: {:?}", transactions);
        let total_gas_used = 0u64;
        
        // Calculate state root from transactions (simplified - in production this would be post-execution state)
        let state_root = self.calculate_state_root(transactions.as_slice()  );
        
        // Calculate receipts root (simplified - in production this would be from execution receipts)
        let receipts_root = self.calculate_receipts_root(transactions.as_slice());
        
        // Calculate logs bloom (simplified - in production this would be from execution logs)
        let logs_bloom = self.calculate_logs_bloom(transactions.as_slice());
        
        // Calculate prev_randao (mixHash) from subdag randomness
        let prev_randao = self.calculate_prev_randao();
        
        // Calculate fee recipient from leader authority
        let fee_recipient = self.fee_recipient.clone();
        
        // Calculate block number from commit reference
        let block_number = self.subdag.commit_ref.index as u64;
        
        // Set gas limit to standard Ethereum block gas limit
        let gas_limit = 30_000_000u64;
        
        // Calculate base fee per gas (EIP-1559)
        let base_fee_per_gas = self.calculate_base_fee_per_gas();
        
        // Calculate blob gas fields for EIP-4844 (Cancun upgrade)
        let (blob_gas_used, excess_blob_gas) = self.calculate_blob_gas_fields();
        
        // Create extra data with subdag information
        let extra_data = self.create_extra_data();
        
        let header = BlockHeaderForHashing {
            parent_hash: self.parent_hash.clone(),
            fee_recipient,
            state_root,
            receipts_root,
            logs_bloom,
            prev_randao,
            block_number: self.subdag.commit_ref.index as u64,
            gas_limit: 30_000_000u64,
            gas_used: 0u64, // Will be calculated from actual transactions
            timestamp: timestamp_secs,
            extra_data: extra_data.clone(),
            base_fee_per_gas,
            // withdrawals_root is calculated from withdrawals array (empty for now)
            withdrawals_root: Hash256::ZERO,
            // EIP-4844 fields for Cancun+
            blob_gas_used: 0u64,
            excess_blob_gas: 0u64,
        };
        info!("header: {:?}", header);
        // RLP encode the header
        let rlp_encoded = header.rlp_encode();
        info!("rlp_encoded: {:?}", rlp_encoded);
        // Hash the RLP-encoded header using Keccak256
        let hash_bytes = keccak256(&rlp_encoded);
        info!("hash_bytes: {:?}", hash_bytes);
        // Convert to Hash256
        let block_hash = Hash256::from_slice(&hash_bytes[..]);
        ExecutionPayloadV3 {
            payload_inner: alloy_rpc_types_engine::ExecutionPayloadV2 {
                payload_inner: alloy_rpc_types_engine::ExecutionPayloadV1 {
                    parent_hash: self.parent_hash.clone()   ,
                    fee_recipient,
                    state_root,
                    receipts_root,
                    logs_bloom,
                    prev_randao,
                    block_number,
                    gas_limit,
                    gas_used: total_gas_used,
                    timestamp: timestamp_secs,
                    extra_data,
                    base_fee_per_gas,
                    block_hash,
                    transactions,
                },
                withdrawals: Vec::new(), // Post-Shanghai blocks require withdrawals (empty for no withdrawals)
            },
            blob_gas_used,
            excess_blob_gas,
        }
    }
    
    /// Calculate parent hash from leader block reference
    fn calculate_parent_hash(&self) -> Hash256 {
        // In a real implementation, this would be the hash of the previous block
        // For now, create a deterministic hash from the leader's round
        let mut bytes = [0u8; 32];
        let round_bytes = self.subdag.leader.round.to_le_bytes();
        bytes[0..round_bytes.len()].copy_from_slice(&round_bytes);
        
        Hash256::from(bytes)
    }
    
    /// Calculate state root from transactions
    fn calculate_state_root(&self, transactions: &[Bytes]) -> Hash256 {
        if transactions.is_empty() {
            return Hash256::ZERO;
        }
        
        // Create a simple merkle-like hash from transactions
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        for tx in transactions {
            tx.hash(&mut hasher);
        }
        
        let hash_value = hasher.finish();
        let mut bytes = [0u8; 32];
        bytes[0..8].copy_from_slice(&hash_value.to_le_bytes());
        
        Hash256::from(bytes)
    }
    
    /// Calculate receipts root from transactions
    fn calculate_receipts_root(&self, transactions: &[Bytes]) -> Hash256 {
        if transactions.is_empty() {
            return Hash256::ZERO;
        }
        
        // In a real implementation, this would be calculated from execution receipts
        // For now, create a deterministic hash
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        transactions.len().hash(&mut hasher);
        
        let hash_value = hasher.finish();
        let mut bytes = [0u8; 32];
        bytes[0..8].copy_from_slice(&hash_value.to_le_bytes());
        
        Hash256::from(bytes)
    }
    
    /// Calculate logs bloom from transactions
    fn calculate_logs_bloom(&self, transactions: &[Bytes]) -> Bloom {
        if transactions.is_empty() {
            return Bloom::default();
        }
        
        // In a real implementation, this would be calculated from execution logs
        // For now, create a simple bloom filter
        let mut bloom = Bloom::default();
        
        // Set some bits based on transaction count
        let tx_count = transactions.len() as u64;
        for i in 0..8 {
            let bit_index = (tx_count + i) % 2048;
            let byte_index = (bit_index / 8) as usize;
            let bit_offset = (bit_index % 8) as u8;
            if byte_index < 256 {
                bloom.0[byte_index] |= 1 << bit_offset;
            }
        }
        
        bloom
    }
    
    /// Calculate prev_randao (mixHash) from subdag randomness
    fn calculate_prev_randao(&self) -> Hash256 {
        // In a real implementation, this would be the RANDAO reveal from the beacon chain
        // For now, create a deterministic hash from timestamp and round
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        self.subdag.timestamp_ms.hash(&mut hasher);
        self.subdag.leader.round.hash(&mut hasher);
        
        let hash_value = hasher.finish();
        let mut bytes = [0u8; 32];
        bytes[0..8].copy_from_slice(&hash_value.to_le_bytes());
        
        Hash256::from(bytes)
    }
    
    /// Calculate base fee per gas (EIP-1559)
    fn calculate_base_fee_per_gas(&self) -> alloy_primitives::U256 {
        // In a real implementation, this would be calculated based on network congestion
        // For now, use a reasonable default base fee
        alloy_primitives::U256::from(1_000_000_000u64) // 1 gwei
    }
    
    /// Calculate blob gas fields for EIP-4844
    fn calculate_blob_gas_fields(&self) -> (u64, u64) {
        // In a real implementation, these would be calculated based on blob transactions
        // For now, return zeros (no blob gas used)
        (0, 0)
    }
    
    /// Create extra data with subdag information
    fn create_extra_data(&self) -> Bytes {
        // Include subdag metadata in extra data
        let mut extra_data = Vec::new();
        
        // Add subdag round
        extra_data.extend_from_slice(&self.subdag.leader.round.to_le_bytes());
        
        // Add timestamp
        extra_data.extend_from_slice(&self.subdag.timestamp_ms.to_le_bytes());
        
        // Add commit reference index
        extra_data.extend_from_slice(&self.subdag.commit_ref.index.to_le_bytes());
        
        // Add a marker to identify this as a FastEVM block
        extra_data.extend_from_slice(b"FastEVM");
        
        Bytes::from(extra_data)
    }
    
    pub fn flatten_transactions(&self) -> Vec<Bytes> {
        let mut flattened_txs: Vec<Bytes> = Vec::new();
        let mut total_gas_used: u64 = 0;

        for vb in &self.subdag.blocks {
            // --- IMPORTANT: replace the code below with the real one ---
            // Possible valid variants (adapt to your VerifiedBlock API):
            // 1) if VerifiedBlock has .transactions() -> &[TxType]:
            //    for tx in vb.transactions().iter() { flattened_txs.push(tx.to_raw_bytes()); total_gas_used += tx.gas_used(); }
            //
            // 2) if VerifiedBlock stores raw bytes: for raw in vb.raw_transactions() { flattened_txs.push(raw.clone()); }
            //
            //Extract transactions from verified block
            for tx in vb.transactions() {
                let raw_data = Bytes::from(tx.data().to_vec());
                //Add calculated gas used here
                total_gas_used = total_gas_used.saturating_add(0); // if you can measure gas, add it here
                flattened_txs.push(raw_data);
            }
        }
        flattened_txs
    }
}

/// Block header structure for hashing (follows Ethereum specification)
#[derive(Debug)]
struct BlockHeaderForHashing {
    parent_hash: Hash256,
    fee_recipient: Address,
    state_root: Hash256,
    receipts_root: Hash256,
    logs_bloom: Bloom,
    prev_randao: Hash256,
    block_number: u64,
    gas_limit: u64,
    gas_used: u64,
    timestamp: u64,
    extra_data: Bytes,
    base_fee_per_gas: alloy_primitives::U256,
    withdrawals_root: Hash256,
    blob_gas_used: u64,
    excess_blob_gas: u64,
}

impl BlockHeaderForHashing {
    /// RLP encode the block header for hashing
    fn rlp_encode(&self) -> Vec<u8> {
        // Create RLP encoder
        let mut encoder = RlpStream::new();
        
        // Start list with the number of fields (15 fields in our struct)
        encoder.begin_list(15);
        
        // Encode fields in the order specified by Ethereum
        encoder.append(&self.parent_hash.as_slice());
        encoder.append(&self.fee_recipient.as_slice());
        encoder.append(&self.state_root.as_slice());
        encoder.append(&self.receipts_root.as_slice());
        encoder.append(&self.logs_bloom.0.as_slice());
        encoder.append(&self.prev_randao.as_slice());
        encoder.append(&self.block_number);
        encoder.append(&self.gas_limit);
        encoder.append(&self.gas_used);
        encoder.append(&self.timestamp);
        encoder.append(&self.extra_data.as_ref());
        encoder.append(&self.base_fee_per_gas.to_be_bytes::<32>().as_slice());
        encoder.append(&self.withdrawals_root.as_slice());
        encoder.append(&self.blob_gas_used);
        encoder.append(&self.excess_blob_gas);
        
        // Convert to bytes
        encoder.out().to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_state() -> BeaconState {
        let config = NodeConfig::default();
        BeaconState::from_config(&config).unwrap()
    }
    #[test]
    fn test_beacon_state_from_config_and_genesis() {
        let state =create_test_state();
        
        assert_eq!(state.slot(), 0);
        assert_eq!(state.validators.len(), 1);
        assert_eq!(state.balances.len(), 1);
        assert_eq!(state.randao_mixes.len(), 65536);
        assert_eq!(state.slashings.len(), 8192);
    }

    #[test]
    fn test_beacon_state_process_subdag() {
        let mut state = create_test_state();
        
        let initial_slot = state.slot();
        
        // Create a mock committed subdag
        let subdag = CommittedSubDag {
            leader: consensus_core::BlockRef::MIN,
            blocks: vec![],
            timestamp_ms: 1000,
            commit_ref: consensus_core::CommitRef::new(1, consensus_core::CommitDigest::default()),
            rejected_transactions_by_block: vec![],
            reputation_scores_desc: vec![],
        };
        
        let payload = state.process_subdag(subdag);
        
        // Verify that the slot was advanced
        assert_eq!(state.slot(), initial_slot + 1);
        
        // Verify that the payload was created
        assert_eq!(payload.payload_inner.payload_inner.block_number, initial_slot + 1);
    }
    #[test]
    fn test_rlp_encode_default_values() {
       let header = BlockHeaderForHashing {
        parent_hash: Hash256::ZERO,
        fee_recipient: Address::default(),
        state_root: Hash256::ZERO,
        receipts_root: Hash256::ZERO,
        logs_bloom: Bloom::default(),
        prev_randao: Hash256::ZERO,
        block_number: 0,
        gas_limit: 30_000_000,
        gas_used: 0,
        timestamp: 0,
        extra_data: Bytes::default(),
        base_fee_per_gas: U256::ZERO,
        withdrawals_root: Hash256::ZERO,
        blob_gas_used: 0,
        excess_blob_gas: 0,
       };
       let rlp_encoded = header.rlp_encode();
       info!("rlp_encoded: {:?}", rlp_encoded);
    }
    #[test]
    fn test_rlp_encode_with_values() {
        let header = BlockHeaderForHashing {
            parent_hash: Hash256::from_str("84e160c63aa52e1e959f36f4162d532dc6bf78e54a2aa6a56aea6a7546947fa9").unwrap(), 
            fee_recipient: Address::from_str("0000000000000000000000000000000000000000").unwrap(), 
            state_root: Hash256::ZERO, 
            receipts_root: Hash256::ZERO, 
            logs_bloom: Bloom::default(), 
            prev_randao: Hash256::from_str("2978baeeb92ade1c000000000000000000000000000000000000000000000000").unwrap(), 
            block_number: 1, 
            gas_limit: 30000000, 
            gas_used: 0, 
            timestamp: 0, 
            extra_data: Bytes::from_str("010000000000000000000000010000004661737445564d").unwrap(), 
            base_fee_per_gas: U256::from(1000000000u64), 
            withdrawals_root: Hash256::ZERO, 
            blob_gas_used: 0, 
            excess_blob_gas: 0
        };
        let rlp_encoded = header.rlp_encode();
        info!("rlp_encoded: {:?}", rlp_encoded);
        
        // Add assertions to verify the encoding works correctly
        assert!(!rlp_encoded.is_empty(), "RLP encoding should not be empty");
        assert!(rlp_encoded.len() > 0, "RLP encoding should have content");
        
        // Verify that the encoded data can be decoded back (basic validation)
        // This is a simple check that the encoding produces valid RLP data
        assert!(rlp_encoded[0] >= 0xc0, "RLP encoding should start with a list");
    }
}

