use reth_ethereum::node::api::PayloadTypes;
use reth_ethereum_engine_primitives::EthPayloadTypes;
use reth_extension::MysticetiCommittedSubdag;
use reth_payload_builder::PayloadId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProposalBlockStatus {
    Processing,
    Executed,
}

/// Store proposal block for next block
/// This struct is built with 2 phases:
/// 1. Get payloadId and payload attributes, then trigger build payload process
/// 2. After build payload process is finished, update payload
pub(crate) struct ProposalBlock<Payload = EthPayloadTypes>
where
    Payload: PayloadTypes,
{
    // Payload id
    pub payload_id: PayloadId,
    // Payload value is built by payload builder
    pub built_payload: Option<Payload::BuiltPayload>,
    // Attributes is used as input to build payload
    pub attributes: Payload::PayloadAttributes,
    // Whether the payload is built
    pub status: ProposalBlockStatus,
}

impl<Payload> ProposalBlock<Payload>
where
    Payload: PayloadTypes,
{
    pub fn new(payload_id: PayloadId, attributes: Payload::PayloadAttributes) -> Self {
        Self {
            payload_id,
            attributes,
            built_payload: None,
            status: ProposalBlockStatus::Processing,
        }
    }

    pub fn set_payload(&mut self, built_payload: Payload::BuiltPayload) {
        self.built_payload = Some(built_payload);
    }

    pub fn set_executed(&mut self) {
        self.status = ProposalBlockStatus::Executed;
    }

    pub fn is_executed(&self) -> bool {
        self.status == ProposalBlockStatus::Executed
    }
}

// #[derive(Debug, Clone, Copy)]
// pub struct CommittedSugDagInfo {
//     index: u32,
//     timestamp: u64,
//     round: u32,
// }

// impl<Transaction> From<&CommittedTransactions<Transaction>> for CommittedSugDagInfo {
//     fn from(committed_transactions: &CommittedTransactions<Transaction>) -> Self {
//         Self {
//             index: committed_transactions.commit_ref.index,
//             timestamp: committed_transactions.timestamp_ms,
//             round: committed_transactions.leader.round,
//         }
//     }
// }
pub struct BatchCommittedSubDag<Transaction> {
    pub first_committed_subdag: MysticetiCommittedSubdag<Transaction>,
    pub last_committed_subdag: MysticetiCommittedSubdag<Transaction>,
}
