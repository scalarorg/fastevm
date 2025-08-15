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
    _node_index: u32,
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
            _node_index: node_index,
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
        self.current_round = committed_subdag.leader.round;
        let _rewards = self.calculate_rewards(&committed_subdag);
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
    pub fn calculate_rewards(&self, _committed_subdag: &CommittedSubDag) -> HashMap<Address, u64> {
        //TODO: Implement this
        let rewards = HashMap::new();
        
        rewards
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use consensus_core::{BlockRef, CommitDigest, CommitRef, CommittedSubDag};
    use std::collections::HashMap;
    use std::str::FromStr;

    // Mock data structures for testing
    #[derive(Debug, Clone)]
    struct MockVerifiedBlock {
        data: Vec<u8>,
    }

    impl MockVerifiedBlock {
        fn new(data: Vec<u8>) -> Self {
            Self { data }
        }

        fn data(&self) -> &[u8] {
            &self.data
        }
    }

    #[derive(Debug, Clone)]
    struct MockBlockRef {
        round: u32,
        author: u32,
    }

    impl MockBlockRef {
        fn new(round: u32, author: u32) -> Self {
            Self { round, author }
        }
    }

    #[derive(Debug, Clone)]
    struct MockCommittedSubDag {
        leader: MockBlockRef,
        blocks: Vec<MockVerifiedBlock>,
        timestamp_ms: u64,
        commit_ref: MockBlockRef,
    }

    impl MockCommittedSubDag {
        fn new(leader: MockBlockRef, blocks: Vec<MockVerifiedBlock>, timestamp_ms: u64, commit_ref: MockBlockRef) -> Self {
            Self {
                leader,
                blocks,
                timestamp_ms,
                commit_ref,
            }
        }
    }

    // Helper function to create test BlockRef
    fn create_test_block_ref(round: u32) -> BlockRef {
        let mut block_ref = BlockRef::MIN;
        block_ref.round = round;
        block_ref
    }
    fn create_test_commit_ref(round: u32) -> CommitRef {
        CommitRef::new(round, CommitDigest::default())
    }
    #[test]
    fn test_engine_api_config_default() {
        let config = EngineApiConfig::default();
        
        assert_eq!(config.execution_url, "http://127.0.0.1:8551");
        assert_eq!(config.jwt_secret, String::default());
        assert_eq!(config.fee_recipient, String::default());
        assert_eq!(config.poll_interval_ms, 1000);
    }

    #[test]
    fn test_engine_api_config_custom() {
        let config = EngineApiConfig {
            execution_url: "http://localhost:8080".to_string(),
            jwt_secret: "secret123".to_string(),
            fee_recipient: "0x1234567890abcdef".to_string(),
            poll_interval_ms: 5000,
        };
        
        assert_eq!(config.execution_url, "http://localhost:8080");
        assert_eq!(config.jwt_secret, "secret123");
        assert_eq!(config.fee_recipient, "0x1234567890abcdef");
        assert_eq!(config.poll_interval_ms, 5000);
    }

    #[test]
    fn test_consensus_state_new() {
        let fee_recipient = Address::from_str("0x742d35Cc6634C0532925a3b8D4C9db96C4b4d8b6").unwrap();
        let state = ConsensusState::new(42, fee_recipient);
        
        assert_eq!(state._node_index(), &42);
        assert_eq!(state.current_round(), &0);
        assert_eq!(state.current_block_number(), &0);
        assert_eq!(state.fee_recipient(), &fee_recipient);
        assert_eq!(state.finalized_block_hash(), &B256::default());
    }

    #[test]
    fn test_consensus_state_default() {
        let state = ConsensusState::default();
        
        assert_eq!(state._node_index(), &0);
        assert_eq!(state.current_round(), &0);
        assert_eq!(state.current_block_number(), &0);
        assert_eq!(state.fee_recipient(), &Address::default());
        assert_eq!(state.finalized_block_hash(), &B256::default());
    }

    #[test]
    fn test_consensus_state_process_subdag() {
        let fee_recipient = Address::from_str("0x742d35Cc6634C0532925a3b8D4C9db96C4b4d8b6").unwrap();
        let mut state = ConsensusState::new(0, fee_recipient);
        
        // Create a mock CommittedSubDag
        let leader = create_test_block_ref(5);
        let blocks = vec![];
        let commit_ref = create_test_commit_ref(5);
        let subdag = CommittedSubDag {
            leader,
            blocks,
            timestamp_ms: 1000,
            commit_ref,
            rejected_transactions_by_block: Vec::new(),
            reputation_scores_desc: Vec::new(),
        };
        
        let payload = state.process_subdag(subdag);
        
        // Verify the state was updated
        assert_eq!(state.current_round(), &5);
        assert_eq!(state.current_block_number(), &1);
        assert_eq!(state.fee_recipient(), &fee_recipient);
        
        // Verify the payload was created correctly
        assert_eq!(payload.execution_payload.fee_recipient, fee_recipient);
        assert_eq!(payload.execution_payload.timestamp, 1); // timestamp_ms / 1000
        assert_eq!(payload.execution_payload.block_number, 1);
        assert_eq!(payload.execution_payload.parent_hash, B256::default());
        assert_eq!(payload.withdrawals, None);
    }

    #[test]
    fn test_consensus_state_process_multiple_subdags() {
        let fee_recipient = Address::from_str("0x742d35Cc6634C0532925a3b8D4C9db96C4b4d8b6").unwrap();
        let mut state = ConsensusState::new(0, fee_recipient);
        
        // Process first subdag
        let leader1 = create_test_block_ref(1);
        let subdag1 = CommittedSubDag {
            leader: leader1,
            blocks: vec![],
            timestamp_ms: 1000,
            commit_ref: create_test_commit_ref(1),
            rejected_transactions_by_block: Vec::new(),
            reputation_scores_desc: Vec::new(),
        };
        
        let payload1 = state.process_subdag(subdag1);
        assert_eq!(state.current_block_number(), &1);
        assert_eq!(payload1.execution_payload.block_number, 1);
        
        // Process second subdag
        let leader2 = create_test_block_ref(2);
        let subdag2 = CommittedSubDag {
            leader: leader2,
            blocks: vec![],
            timestamp_ms: 2000,
            commit_ref: create_test_commit_ref(2),
            rejected_transactions_by_block: Vec::new(),
            reputation_scores_desc: Vec::new(),
        };
        
        let payload2 = state.process_subdag(subdag2);
        assert_eq!(state.current_block_number(), &2);
        assert_eq!(payload2.execution_payload.block_number, 2);
        assert_eq!(state.current_round(), &2);
    }

    #[test]
    fn test_consensus_state_calculate_rewards() {
        let fee_recipient = Address::from_str("0x742d35Cc6634C0532925a3b8D4C9db96C4b4d8b6").unwrap();
        let state = ConsensusState::new(0, fee_recipient);
        
        let leader = create_test_block_ref(1);
        let subdag = CommittedSubDag {
            leader,
            blocks: vec![],
            timestamp_ms: 1000,
            commit_ref: create_test_commit_ref(1),
            rejected_transactions_by_block: Vec::new(),
            reputation_scores_desc: Vec::new(),
        };
        
        let rewards = state.calculate_rewards(&subdag);
        
        // Currently returns empty HashMap as per TODO comment
        assert!(rewards.is_empty());
    }

    #[test]
    fn test_consensus_state_edge_cases() {
        let fee_recipient = Address::from_str("0x742d35Cc6634C0532925a3b8D4C9db96C4b4d8b6").unwrap();
        let mut state = ConsensusState::new(0, fee_recipient);
        
        // Test with zero timestamp
        let leader = create_test_block_ref(0);
        let subdag = CommittedSubDag {
            leader,
            blocks: vec![],
            timestamp_ms: 0,
            commit_ref: create_test_commit_ref(0),
            rejected_transactions_by_block: Vec::new(),
            reputation_scores_desc: Vec::new(),
        };
        
        let payload = state.process_subdag(subdag);
        assert_eq!(payload.execution_payload.timestamp, 0);
        assert_eq!(state.current_block_number(), &1);
        
        // Test with very large timestamp
        let leader2 = create_test_block_ref(1);
        let subdag2 = CommittedSubDag {
            leader: leader2,
            blocks: vec![],
            timestamp_ms: u64::MAX,
            commit_ref: create_test_commit_ref(1),
            rejected_transactions_by_block: Vec::new(),
            reputation_scores_desc: Vec::new(),
        };
        
        let payload2 = state.process_subdag(subdag2);
        assert_eq!(payload2.execution_payload.timestamp, u64::MAX / 1000);
        assert_eq!(state.current_block_number(), &2);
    }

    #[test]
    fn test_consensus_state_fee_recipient_persistence() {
        let fee_recipient = Address::from_str("0x742d35Cc6634C0532925a3b8D4C9db96C4b4d8b6").unwrap();
        let mut state = ConsensusState::new(0, fee_recipient);
        
        // Process multiple subdags to ensure fee_recipient persists
        for i in 1..=5 {
            let leader = create_test_block_ref(i);
            let subdag = CommittedSubDag {
                leader,
                blocks: vec![],
                timestamp_ms: i as u64 * 1000,
                commit_ref: create_test_commit_ref(i),
                rejected_transactions_by_block: Vec::new(),
                reputation_scores_desc: Vec::new(),
            };
            
            let payload = state.process_subdag(subdag);
            assert_eq!(payload.execution_payload.fee_recipient, fee_recipient);
        }
        
        assert_eq!(state.fee_recipient(), &fee_recipient);
    }

    #[test]
    fn test_consensus_state_block_hash_chain() {
        let fee_recipient = Address::from_str("0x742d35Cc6634C0532925a3b8D4C9db96C4b4d8b6").unwrap();
        let mut state = ConsensusState::new(0, fee_recipient);
        
        let leader1 = create_test_block_ref(1);
        let subdag1 = CommittedSubDag {
            leader: leader1,
            blocks: vec![],
            timestamp_ms: 1000,
            commit_ref: create_test_commit_ref(1),
            rejected_transactions_by_block: Vec::new(),
            reputation_scores_desc: Vec::new(),
        };
        
        let payload1 = state.process_subdag(subdag1);
        let block_hash1 = payload1.execution_payload.block_hash;
        
        // Process second subdag
        let leader2 = create_test_block_ref(2);
        let subdag2 = CommittedSubDag {
            leader: leader2,
            blocks: vec![],
            timestamp_ms: 2000,
            commit_ref: create_test_commit_ref(2),
            rejected_transactions_by_block: Vec::new(),
            reputation_scores_desc: Vec::new(),
        };
        
        let payload2 = state.process_subdag(subdag2);
        
        // Verify parent_hash points to previous block
        assert_eq!(payload2.execution_payload.parent_hash, block_hash1);
    }

    #[test]
    fn test_consensus_state_round_progression() {
        let fee_recipient = Address::from_str("0x742d35Cc6634C0532925a3b8D4C9db96C4b4d8b6").unwrap();
        let mut state = ConsensusState::new(0, fee_recipient);
        
        let rounds = vec![1, 5, 10, 100, 1000];
        
        for (i, round) in rounds.iter().enumerate() {
            let leader = create_test_block_ref(*round);
            let subdag = CommittedSubDag {
                leader,
                blocks: vec![],
                timestamp_ms: *round as u64 * 1000,
                commit_ref: create_test_commit_ref(*round),
                rejected_transactions_by_block: Vec::new(),
                reputation_scores_desc: Vec::new(),
            };
            
            let _payload = state.process_subdag(subdag);
            assert_eq!(state.current_round(), round);
            assert_eq!(state.current_block_number(), &((i + 1) as u64));
        }
    }

    #[test]
    fn test_consensus_state_with_different_node_indices() {
        let fee_recipient = Address::from_str("0x742d35Cc6634C0532925a3b8D4C9db96C4b4d8b6").unwrap();
        
        for node_index in [0, 1, 10, 100, 1000] {
            let state = ConsensusState::new(node_index, fee_recipient);
            assert_eq!(state._node_index(), &node_index);
            assert_eq!(state.fee_recipient(), &fee_recipient);
        }
    }

    #[test]
    fn test_consensus_state_clone() {
        let fee_recipient = Address::from_str("0x742d35Cc6634C0532925a3b8D4C9db96C4b4d8b6").unwrap();
        let mut state = ConsensusState::new(42, fee_recipient);
        
        // Process a subdag to change the state
        let leader = create_test_block_ref(1);
        let subdag = CommittedSubDag {
            leader,
            blocks: vec![],
            timestamp_ms: 1000,
            commit_ref: create_test_commit_ref(1),
            rejected_transactions_by_block: Vec::new(),
            reputation_scores_desc: Vec::new(),
        };
        
        let _payload = state.process_subdag(subdag);
        
        // Clone the state
        let cloned_state = state.clone();
        
        // Verify all fields are cloned correctly
        assert_eq!(cloned_state._node_index(), state._node_index());
        assert_eq!(cloned_state.current_round(), state.current_round());
        assert_eq!(cloned_state.current_block_number(), state.current_block_number());
        assert_eq!(cloned_state.fee_recipient(), state.fee_recipient());
        assert_eq!(cloned_state.finalized_block_hash(), state.finalized_block_hash());
    }

    #[test]
    fn test_consensus_state_debug() {
        let fee_recipient = Address::from_str("0x742d35Cc6634C0532925a3b8D4C9db96C4b4d8b6").unwrap();
        let state = ConsensusState::new(42, fee_recipient);
        
        // Test that debug formatting works
        let debug_str = format!("{:?}", state);
        assert!(debug_str.contains("42"));
        assert!(debug_str.contains("0"));
    }
}

