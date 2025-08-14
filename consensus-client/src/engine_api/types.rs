use std::collections::HashMap;

use alloy_primitives::{Address, B256};
use alloy_rpc_types_engine::ExecutionPayloadInputV2;
use consensus_core::CommittedSubDag;
use derive_getters::Getters;

use crate::engine_api::SubDagBlock;
/// Engine API client configuration
#[derive(Debug, Clone)]
pub struct EngineApiConfig {
    pub execution_url: String,
    //Secret in hex with 64 bytes length
    pub jwt_secret: String,
    pub fee_recipient: String,
    pub poll_interval_ms: u64,
}

impl Default for EngineApiConfig {
    fn default() -> Self {
        Self {
            execution_url: "http://127.0.0.1:8551".to_string(),
            jwt_secret: String::default(),
            fee_recipient: String::default(),
            poll_interval_ms: 1000,
        }
    }
}

/// Consensus state for tracking finality
#[derive(Default, Debug, Clone, Getters)]
pub struct ConsensusState {
    node_index: u32,
    current_round: u32,
    current_block_number: u64,    
    fee_recipient: Address,   
    finalized_block_hash: B256,
    // pub finalized_subdags: HashMap<String, SubDAG>,
    // pub pending_subdags: HashMap<String, SubDAG>,
    // pub latest_finalized_block: Option<H256>,
    // pub validators: Vec<NodeId>,
    // pub current_leader: Option<NodeId>,
}
impl ConsensusState {
    pub fn new(node_index: u32, fee_recipient: Address) -> Self {
        Self {
            node_index,
            current_round: 0,
            current_block_number: 0,            
            fee_recipient,
            finalized_block_hash: B256::default(),            
        }
    }
    ///
    /// Process the consensus output and return the execution payload input v2
    /// 
    pub fn process_subdag(&mut self, committed_subdag: CommittedSubDag) -> ExecutionPayloadInputV2 {
        //Set current rount to the leader round
        self.current_round = committed_subdag.leader.round.clone();
        let rewards = self.calculate_rewards(&committed_subdag);
        let block = SubDagBlock::new(committed_subdag);
        let mut payload = block.get_execution_payload_input_v2();
        let block_hash = block.get_block_hash();
        payload.execution_payload.fee_recipient = self.fee_recipient;
        payload.execution_payload.timestamp = block.get_block_timestamp();
        payload.execution_payload.block_number = self.current_block_number + 1;
        payload.execution_payload.block_hash = block_hash.clone();
        payload.execution_payload.parent_hash = self.finalized_block_hash.clone();
        self.current_block_number += 1;
        self.finalized_block_hash = block_hash;        
        payload
    }
    pub fn calculate_rewards(&self, committed_subdag: &CommittedSubDag) -> HashMap<Address, u64> {
        //TODO: Implement this
        let mut rewards = HashMap::new();
        
        rewards
    }
}

