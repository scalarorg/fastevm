use alloy_primitives::B256;
use derive_getters::Getters;
/// Engine API client configuration
#[derive(Debug, Clone)]
pub struct EngineApiConfig {
    pub execution_url: String,
    //Secret in hex with 64 bytes length
    pub jwt_secret: String,
    pub poll_interval_ms: u64,
}

impl Default for EngineApiConfig {
    fn default() -> Self {
        Self {
            execution_url: "http://127.0.0.1:8551".to_string(),
            jwt_secret: String::default(),
            poll_interval_ms: 1000,
        }
    }
}

/// Consensus state for tracking finality
#[derive(Debug, Clone, Getters)]
pub struct ConsensusState {
    current_round: u64,
    finalized_block_hash: B256,
    // pub finalized_subdags: HashMap<String, SubDAG>,
    // pub pending_subdags: HashMap<String, SubDAG>,
    // pub latest_finalized_block: Option<H256>,
    // pub validators: Vec<NodeId>,
    // pub current_leader: Option<NodeId>,
}

impl Default for ConsensusState {
    fn default() -> Self {
        Self {
            current_round: 0,
            finalized_block_hash: B256::default(),
            // finalized_subdags: HashMap::new(),
            // pending_subdags: HashMap::new(),
            // latest_finalized_block: None,
            // validators: Vec::new(),
            // current_leader: None,
        }
    }
}
