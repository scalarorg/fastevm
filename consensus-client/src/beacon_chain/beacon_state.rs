use std::collections::HashMap;
use std::str::FromStr;
use alloy_rpc_types_engine::PayloadError;
use anyhow::anyhow;
use alloy_primitives::{Address, B256};
use alloy_rpc_types_engine::{ ExecutionData, ForkchoiceState, PayloadAttributes};

use consensus_core::CommittedSubDag;
use tree_hash::{Hash256};
use serde::{Deserialize, Serialize};

use crate::{beacon_chain::{BeaconBlockHeader, SubDagBlock}, NodeConfig};

// Main Ethereum 2.0 Beacon State Implementation
/// Main beacon state structure following Ethereum 2.0 specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeaconState {
    pub fee_recipient: Address,
    // Genesis
    pub genesis_time: u64,
    pub genesis_validators_root: Hash256,
    pub slot: u64,
    
    // History
    pub latest_block_header: BeaconBlockHeader,
    pub block_roots: Vec<Hash256>,
    pub state_roots: Vec<Hash256>,
    pub historical_roots: Vec<Hash256>,
    
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
    
    // Inactivity
    pub inactivity_scores: Vec<u64>,
    
    // Sync
    pub current_sync_committee: SyncCommittee,
    pub next_sync_committee: SyncCommittee,

}

impl BeaconState {
    //Initializes the consensus state with the genesis block hash get from the execution client init
    /// Create a new genesis state
    pub fn new(genesis_time: u64, fee_recipient: Address) -> Self {
        Self {
            genesis_time,
            fee_recipient,
            genesis_validators_root: Hash256::default(),
            slot: 0,
            latest_block_header: BeaconBlockHeader::default(),
            block_roots: Vec::new(),
            state_roots: Vec::new(),
            historical_roots: Vec::new(),
            validators: Vec::new(),
            balances: Vec::new(),
            randao_mixes: Vec::new(),
            slashings: Vec::new(),
            previous_epoch_participation: Vec::new(),
            current_epoch_participation: Vec::new(),
            inactivity_scores: Vec::new(),
            current_sync_committee: SyncCommittee::default(),
            next_sync_committee: SyncCommittee::default(),
        }
    }
    
    /// Create a new genesis state from NodeConfig and GenesisData
    pub fn from_config(config: &NodeConfig)-> Result<Self, anyhow::Error> {
        let fee_recipient = Address::from_str(&config.fee_recipient).map_err(|e| anyhow!("Invalid fee recipient: {:?}", e))?;
        let genesis_block_hash = B256::from_str(config.genesis_block_hash.trim_start_matches("0x")).
            map_err(|e| anyhow!("Invalid genesis block hash: {:?}, {:?}", config.genesis_block_hash, e))?;
    
        let mut state = Self::new(config.genesis_time, fee_recipient);
        state.latest_block_header.parent_root = genesis_block_hash;
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
        
        // Initialize sync committees
        state.current_sync_committee = SyncCommittee {
            pubkeys: vec![Hash256::default()],
            aggregate_pubkey: Hash256::default(),
        };
        state.next_sync_committee = state.current_sync_committee.clone();
        
        Ok(state)
    }

    pub fn get_fork_choice_state(&self) -> ForkchoiceState {
        let block_hash = self.latest_block_header.parent_root.clone();
        ForkchoiceState {
            head_block_hash: block_hash.clone(),
            safe_block_hash: block_hash.clone(),
            finalized_block_hash: block_hash,
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
            parent_beacon_block_root: Some(self.get_parent_beacon_block_root()),
        };
        Some(attributes)
    }
    pub fn get_parent_beacon_block_root(&self) -> B256 {
        self.latest_block_header.canonical_root()
    }
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
    pub fn process_subdag(&mut self, committed_subdag: CommittedSubDag) -> Result<ExecutionData, PayloadError> {
        // Set current slot to the leader round
        self.slot = committed_subdag.leader.round as u64;
        
        // Calculate rewards (simplified)
        let _rewards = self.calculate_rewards(&committed_subdag);
        
        // Create a block from the subdag
        let block = SubDagBlock::new(
            self.fee_recipient(), 
            committed_subdag);
        // Create execution data and update the latest block header, parent_hash for next block

        let payload = block.create_execution_data_v3(&mut self.latest_block_header);
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
    pub fn set_state_root(&mut self, slot: usize, state_root: Hash256) -> Result<(), anyhow::Error> {
        let index = slot % 8192; // SLOTS_PER_HISTORICAL_ROOT
        if index >= self.state_roots.len() {
            self.state_roots.resize(index + 1, Hash256::ZERO);
        }
        self.state_roots[index] = state_root;
        Ok(())
    }

    /// Get the balance at the given index
    pub fn get_balance(&self, index: u64) -> Result<u64, anyhow::Error> {
        if index < self.balances.len() as u64 {
            Ok(self.balances[index as usize])
        } else {
            Err(anyhow::anyhow!("Index out of bounds"))
        }
    }

    /// Get the RANDAO mix for the current epoch
    pub fn get_randao_mix(&self, epoch: u64) -> Result<Hash256, anyhow::Error> {
        let index = epoch % 65536; // EPOCHS_PER_HISTORICAL_VECTOR
        if index < self.randao_mixes.len() as u64 {
            Ok(self.randao_mixes[index as usize])
        } else {
            Err(anyhow::anyhow!("Index out of bounds"))
        }
    }

    /// Set the RANDAO mix for the current epoch
    pub fn set_randao_mix(&mut self, epoch: u64, mix: Hash256) -> Result<(), anyhow::Error> {
        let index = epoch % 65536; // EPOCHS_PER_HISTORICAL_VECTOR
        if index >= self.randao_mixes.len() as u64 {
            self.randao_mixes.resize(index as usize + 1, Hash256::ZERO);
        }
        self.randao_mixes[index as usize] = mix;
        Ok(())
    }

}

// Supporting data structures

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
        
        let ExecutionData { payload, sidecar:_ } = state.process_subdag(subdag).unwrap();
        let payload = payload.as_v3().unwrap();
        // Verify that the slot was advanced
        assert_eq!(state.slot(), initial_slot + 1);
        
        // Verify that the payload was created
        assert_eq!(payload.payload_inner.payload_inner.block_number, initial_slot + 1);
    }
}

