use alloy_primitives::Address;
use derivative::Derivative;
use serde::{Deserialize, Serialize};
use tree_hash::TreeHash;
use tree_hash_derive::TreeHash;
use tree_hash::Hash256;

/// Beacon block header containing essential block information
#[derive(
    Default, Debug, Clone, Serialize, Deserialize, TreeHash, Derivative,
)]
#[derivative(PartialEq, Eq, Hash)]
pub struct BeaconBlockHeader {
    /// Slot number when this block was proposed
    pub slot: u64,
    /// Index of the proposer
    pub proposer_index: u32,
    /// Hash of the parent block
    pub parent_root: Hash256,
    /// Hash of the state root after processing this block
    pub state_root: Hash256,
    /// Hash of the block body
    pub body_root: Hash256,
}

impl BeaconBlockHeader {
    /// Create a new block header
    pub fn new(
        slot: u64,
        proposer_index: u32,
        parent_root: Hash256,
        state_root: Hash256,
        body_root: Hash256,
    ) -> Self {
        Self {
            slot,
            proposer_index,
            parent_root,
            state_root,
            body_root,
        }
    }

    /// Get the canonical root of this block header
    pub fn canonical_root(&self) -> Hash256 {
        self.tree_hash_root()
    }
}

/// Beacon block body containing all block operations
#[derive(
    Default, Debug, Clone, Serialize, Deserialize, Derivative,
)]
#[derivative(PartialEq, Eq, Hash)]
pub struct BeaconBlockBody {
    /// Randomness beacon for this block
    pub randao_reveal: Hash256,
    /// ETH1 data for this block
    pub eth1_data: Eth1Data,
    /// Graffiti data
    pub graffiti: Hash256,
    // Proposer slashings
    pub proposer_slashings: Vec<ProposerSlashing>,
    /// Attester slashings
    pub attester_slashings: Vec<AttesterSlashing>,
    /// Attestations included in this block
    pub attestations: Vec<Attestation>,
    /// Deposits included in this block
    pub deposits: Vec<Deposit>,
    /// Voluntary exits
    pub voluntary_exits: Vec<SignedVoluntaryExit>,
    /// Sync committee aggregate (Altair+)
    pub sync_aggregate: Option<SyncAggregate>,
    /// Execution payload (Merge+)
    pub execution_payload: Option<ExecutionPayload>,
}

impl BeaconBlockBody {
    /// Create a new empty block body
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new block body with basic fields
    pub fn with_basic_fields(
        randao_reveal: Hash256,
        eth1_data: Eth1Data,
        graffiti: Hash256,
    ) -> Self {
        Self {
            randao_reveal,
            eth1_data,
            graffiti,
            ..Default::default()
        }
    }
    
    /// Get a simplified hash of the body
    pub fn hash(&self) -> Hash256 {
        // Simplified hash - in real implementation this would be a proper hash
        let mut result = [0u8; 32];
        for (i, byte) in result.iter_mut().enumerate() {
            *byte = (self.randao_reveal.as_slice()[0] as u8).wrapping_add(i as u8);
        }
        Hash256::from(result)
    }
}

/// Main beacon block structure
#[derive(
    Default, Debug, Clone, Serialize, Deserialize, Derivative,
)]
#[derivative(PartialEq, Eq, Hash)]
pub struct BeaconBlock {
    /// Block header
    pub header: BeaconBlockHeader,
    /// Block body
    pub body: BeaconBlockBody,
}

impl BeaconBlock {
    /// Create a new beacon block
    pub fn new(
        slot: u64,
        proposer_index: u32,
        parent_root: Hash256,
        state_root: Hash256,
        body: BeaconBlockBody,
    ) -> Self {
        let header = BeaconBlockHeader::new(
            slot,
            proposer_index,
            parent_root,
            state_root,
            body.hash(),
        );

        Self {
            header,
            body,
        }
    }

    /// Create an empty block for a given slot
    pub fn empty(_spec: &ChainSpec) -> Self {
        Self::new(
            0,
            0,
            Hash256::ZERO,
            Hash256::ZERO,
            BeaconBlockBody::new(),
        )
    }

    /// Get the slot number
    pub fn slot(&self) -> u64 {
        self.header.slot
    }

    /// Get the proposer index
    pub fn proposer_index(&self) -> u32 {
        self.header.proposer_index
    }

    /// Get the parent root
    pub fn parent_root(&self) -> Hash256 {
        self.header.parent_root
    }

    /// Get the state root
    pub fn state_root(&self) -> Hash256 {
        self.header.state_root
    }

    /// Get the body root
    pub fn body_root(&self) -> Hash256 {
        self.header.body_root
    }

    /// Get a reference to the block body
    pub fn body(&self) -> &BeaconBlockBody {
        &self.body
    }

    /// Get a mutable reference to the block body
    pub fn body_mut(&mut self) -> &mut BeaconBlockBody {
        &mut self.body
    }

    /// Create a temporary block header from this block
    pub fn temporary_block_header(&self) -> BeaconBlockHeader {
        BeaconBlockHeader {
            slot: self.slot(),
            proposer_index: self.proposer_index(),
            parent_root: self.parent_root(),
            state_root: self.state_root(),
            body_root: self.body().hash(),
        }
    }

    /// Verify that this block is valid for the given slot
    pub fn verify_slot(&self, expected_slot: u64) -> Result<(), BlockError> {
        if self.slot() != expected_slot {
            return Err(BlockError::SlotMismatch {
                expected: expected_slot,
                got: self.slot(),
            });
        }
        Ok(())
    }

    /// Verify that this block's parent matches the expected parent
    pub fn verify_parent(&self, expected_parent: Hash256) -> Result<(), BlockError> {
        if self.parent_root() != expected_parent {
            return Err(BlockError::ParentMismatch {
                expected: expected_parent,
                got: self.parent_root(),
            });
        }
        Ok(())
    }
}

/// Signed beacon block with proposer signature
#[derive(
    Default, Debug, Clone, Serialize, Deserialize, Derivative,
)]
#[derivative(PartialEq, Eq, Hash)]
pub struct SignedBeaconBlock {
    /// The beacon block
    pub message: BeaconBlock,
    /// Proposer signature
    pub signature: Hash256,
}

impl SignedBeaconBlock {
    /// Create a new signed beacon block
    pub fn new(message: BeaconBlock, signature: Hash256) -> Self {
        Self { message, signature }
    }

    /// Create a signed block from an unsigned block
    pub fn from_block(block: BeaconBlock, signature: Hash256) -> Self {
        Self::new(block, signature)
    }

    /// Get the block
    pub fn block(&self) -> &BeaconBlock {
        &self.message
    }

    /// Get the signature
    pub fn signature(&self) -> Hash256 {
        self.signature
    }
}

// Supporting data structures

/// ETH1 data for a block
#[derive(
    Default, Debug, Clone, Serialize, Deserialize, TreeHash, Derivative,
)]
#[derivative(PartialEq, Eq, Hash)]
pub struct Eth1Data {
    /// Deposit root
    pub deposit_root: Hash256,
    /// Deposit count
    pub deposit_count: u64,
    /// Block hash
    pub block_hash: Hash256,
}

/// Proposer slashing
#[derive(
    Default, Debug, Clone, Serialize, Deserialize, Derivative,
)]
#[derivative(PartialEq, Eq, Hash)]
pub struct ProposerSlashing {
    /// First signed header
    pub signed_header_1: SignedBeaconBlockHeader,
    /// Second signed header
    pub signed_header_2: SignedBeaconBlockHeader,
}

/// Attester slashing
#[derive(
    Default, Debug, Clone, Serialize, Deserialize, Derivative,
)]
#[derivative(PartialEq, Eq, Hash)]
pub struct AttesterSlashing {
    /// First attestation
    pub attestation_1: IndexedAttestation,
    /// Second attestation
    pub attestation_2: IndexedAttestation,
}

/// Attestation
#[derive(
    Default, Debug, Clone, Serialize, Deserialize, Derivative,
)]
#[derivative(PartialEq, Eq, Hash)]
pub struct Attestation {
    /// Aggregation bits
    pub aggregation_bits: Hash256,
    /// Attestation data
    pub data: AttestationData,
    /// Aggregate signature
    pub signature: Hash256,
}

/// Attestation data
#[derive(
    Default, Debug, Clone, Serialize, Deserialize, TreeHash, Derivative,
)]
#[derivative(PartialEq, Eq, Hash)]
pub struct AttestationData {
    /// Slot
    pub slot: u64,
    /// Committee index
    pub index: u64,
    /// Beacon block root
    pub beacon_block_root: Hash256,
    /// Source checkpoint
    pub source: Checkpoint,
    /// Target checkpoint
    pub target: Checkpoint,
}

/// Checkpoint
#[derive(
    Default, Debug, Clone, Serialize, Deserialize, TreeHash, Derivative,
)]
#[derivative(PartialEq, Eq, Hash)]
pub struct Checkpoint {
    /// Epoch
    pub epoch: u64,
    /// Root
    pub root: Hash256,
}

/// Indexed attestation
#[derive(
    Default, Debug, Clone, Serialize, Deserialize, Derivative,
)]
#[derivative(PartialEq, Eq, Hash)]
pub struct IndexedAttestation {
    /// Attesting validator indices
    pub attesting_indices: Vec<u64>,
    /// Attestation data
    pub data: AttestationData,
    /// Aggregate signature
    pub signature: Hash256,
}

/// Deposit
#[derive(
    Default, Debug, Clone, Serialize, Deserialize, Derivative,
)]
#[derivative(PartialEq, Eq, Hash)]
pub struct Deposit {
    /// Proof
    pub proof: Vec<Hash256>,
    /// Deposit data
    pub data: DepositData,
}

/// Deposit data
#[derive(
    Default, Debug, Clone, Serialize, Deserialize, Derivative,
)]
#[derivative(PartialEq, Eq, Hash)]
pub struct DepositData {
    /// Public key
    pub pubkey: Hash256,
    /// Withdrawal credentials
    pub withdrawal_credentials: Hash256,
    /// Amount
    pub amount: u64,
    /// Signature
    pub signature: Hash256,
}

/// Voluntary exit
#[derive(
    Default, Debug, Clone, Serialize, Deserialize, Derivative,
)]
#[derivative(PartialEq, Eq, Hash)]
pub struct VoluntaryExit {
    /// Epoch
    pub epoch: u64,
    /// Validator index
    pub validator_index: u64,
}

/// Signed voluntary exit
#[derive(
    Default, Debug, Clone, Serialize, Deserialize, Derivative,
)]
#[derivative(PartialEq, Eq, Hash)]
pub struct SignedVoluntaryExit {
    /// Voluntary exit
    pub message: VoluntaryExit,
    /// Signature
    pub signature: Hash256,
}

/// Sync committee aggregate
#[derive(
    Default, Debug, Clone, Serialize, Deserialize, Derivative,
)]
#[derivative(PartialEq, Eq, Hash)]
pub struct SyncAggregate {
    /// Sync committee bits
    pub sync_committee_bits: Hash256,
    /// Sync committee signature
    pub sync_committee_signature: Hash256,
}

/// Execution payload
#[derive(
    Default, Debug, Clone, Serialize, Deserialize, Derivative,
)]
#[derivative(PartialEq, Eq, Hash)]
pub struct ExecutionPayload {
    /// Parent hash
    pub parent_hash: Hash256,
    /// Fee recipient
    pub fee_recipient: Address,
    /// State root
    pub state_root: Hash256,
    /// Receipts root
    pub receipts_root: Hash256,
    /// Logs bloom
    pub logs_bloom: Hash256,
    /// Previous randao
    pub prev_randao: Hash256,
    /// Block number
    pub block_number: u64,
    /// Gas limit
    pub gas_limit: u64,
    /// Gas used
    pub gas_used: u64,
    /// Timestamp
    pub timestamp: u64,
    /// Extra data
    pub extra_data: Vec<u8>,
    /// Base fee per gas
    pub base_fee_per_gas: u64,
    /// Block hash
    pub block_hash: Hash256,
    /// Transactions
    pub transactions: Vec<Vec<u8>>,
}

/// Signed beacon block header
#[derive(
    Default, Debug, Clone, Serialize, Deserialize, TreeHash, Derivative,
)]
#[derivative(PartialEq, Eq, Hash)]
pub struct SignedBeaconBlockHeader {
    /// Beacon block header
    pub message: BeaconBlockHeader,
    /// Signature
    pub signature: Hash256,
}

// Error types
#[derive(Debug, thiserror::Error)]
pub enum BlockError {
    #[error("Slot mismatch: expected {expected}, got {got}")]
    SlotMismatch { expected: u64, got: u64 },
    #[error("Parent mismatch: expected {expected}, got {got}")]
    ParentMismatch { expected: Hash256, got: Hash256 },
    #[error("Invalid proposer index: {0}")]
    InvalidProposerIndex(u32),
    #[error("Invalid signature")]
    InvalidSignature,
}

/// Chain specification
#[derive(Debug, Clone)]
pub struct ChainSpec {
    pub slots_per_epoch: u64,
    pub slots_per_historical_root: u64,
    pub epochs_per_historical_vector: u64,
    pub epochs_per_slashings_vector: u64,
    pub max_validators_per_committee: u64,
    pub target_committee_size: u64,
    pub max_proposer_slashings: u64,
    pub max_attester_slashings: u64,
    pub max_attestations: u64,
    pub max_deposits: u64,
    pub max_voluntary_exits: u64,
}

impl Default for ChainSpec {
    fn default() -> Self {
        Self {
            slots_per_epoch: 32,
            slots_per_historical_root: 8192,
            epochs_per_historical_vector: 65536,
            epochs_per_slashings_vector: 8192,
            max_validators_per_committee: 2048,
            target_committee_size: 128,
            max_proposer_slashings: 16,
            max_attester_slashings: 2,
            max_attestations: 128,
            max_deposits: 16,
            max_voluntary_exits: 16,
        }
    }
}

impl ChainSpec {
    /// Create a new chain specification
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a custom chain specification
    pub fn custom(
        slots_per_epoch: u64,
        slots_per_historical_root: u64,
        epochs_per_historical_vector: u64,
        epochs_per_slashings_vector: u64,
        max_validators_per_committee: u64,
        target_committee_size: u64,
        max_proposer_slashings: u64,
        max_attester_slashings: u64,
        max_attestations: u64,
        max_deposits: u64,
        max_voluntary_exits: u64,
    ) -> Self {
        Self {
            slots_per_epoch,
            slots_per_historical_root,
            epochs_per_historical_vector,
            epochs_per_slashings_vector,
            max_validators_per_committee,
            target_committee_size,
            max_proposer_slashings,
            max_attester_slashings,
            max_attestations,
            max_deposits,
            max_voluntary_exits,
        }
    }
}

