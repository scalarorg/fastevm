//! Payload component configuration for the Ethereum node.

use crate::{consensus::ConsensusPool, payload::MysticetiPayloadBuilder};
use reth_ethereum::{
    chainspec::{EthChainSpec, EthereumHardforks},
    node::{
        api::{ConfigureEvm, FullNodeTypes, NodeTypes, PayloadTypes, PrimitivesTy, TxTy},
        builder::{components::PayloadBuilderBuilder, BuilderContext, PayloadBuilderConfig},
        engine::EthPayloadAttributes,
    },
    pool::{PoolTransaction, TransactionPool},
    EthPrimitives,
};
use reth_ethereum_payload_builder::EthereumBuilderConfig;
use reth_payload_builder::{EthBuiltPayload, EthPayloadBuilderAttributes};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use tracing::debug;
// use reth_transaction_pool::{PoolTransaction, TransactionPool};

#[non_exhaustive]
pub struct MysticetiPayloadBuilderFactory<Pool, Payload>
where
    Pool: TransactionPool,
    Payload: PayloadTypes,
{
    consensus_pool: Arc<ConsensusPool<Pool>>,
    tx_built_payload: UnboundedSender<Payload::BuiltPayload>,
}

impl<Pool, Payload> MysticetiPayloadBuilderFactory<Pool, Payload>
where
    Pool: TransactionPool,
    Payload: PayloadTypes,
{
    pub fn new(
        consensus_pool: Arc<ConsensusPool<Pool>>,
        tx_built_payload: UnboundedSender<Payload::BuiltPayload>,
    ) -> Self {
        Self {
            consensus_pool,
            tx_built_payload,
        }
    }
}

impl<Types, Node, Pool, Evm, Payload> PayloadBuilderBuilder<Node, Pool, Evm>
    for MysticetiPayloadBuilderFactory<Pool, Payload>
where
    Payload: PayloadTypes<BuiltPayload = EthBuiltPayload>,
    Types: NodeTypes<ChainSpec: EthereumHardforks, Primitives = EthPrimitives>,
    Node: FullNodeTypes<Types = Types>,
    Pool: TransactionPool<Transaction: PoolTransaction<Consensus = TxTy<Node::Types>>>
        + Unpin
        + 'static,
    Evm: ConfigureEvm<
            Primitives = PrimitivesTy<Types>,
            NextBlockEnvCtx = reth_evm::NextBlockEnvAttributes,
        > + 'static,
    Types::Payload: PayloadTypes<
        BuiltPayload = EthBuiltPayload,
        PayloadAttributes = EthPayloadAttributes,
        PayloadBuilderAttributes = EthPayloadBuilderAttributes,
    >,
{
    // type PayloadBuilder =
    //     reth_ethereum_payload_builder::EthereumPayloadBuilder<Pool, Node::Provider, Evm>;
    type PayloadBuilder = MysticetiPayloadBuilder<Pool, Node::Provider, Evm>;
    async fn build_payload_builder(
        self,
        ctx: &BuilderContext<Node>,
        pool: Pool,
        evm_config: Evm,
    ) -> eyre::Result<Self::PayloadBuilder> {
        let conf = ctx.payload_builder_config();
        let chain = ctx.chain_spec().chain();
        let gas_limit = conf.gas_limit_for(chain);
        debug!(target: "build_payload_builder", "Gas limit: {:?}", gas_limit);
        // Ok(reth_ethereum_payload_builder::EthereumPayloadBuilder::new(
        //     ctx.provider().clone(),
        //     pool,
        //     evm_config,
        //     EthereumBuilderConfig::new().with_gas_limit(gas_limit),
        // ))
        let MysticetiPayloadBuilderFactory {
            consensus_pool,
            tx_built_payload,
        } = self;
        Ok(MysticetiPayloadBuilder::new(
            ctx.provider().clone(),
            pool,
            consensus_pool,
            tx_built_payload,
            evm_config,
            EthereumBuilderConfig::new().with_gas_limit(gas_limit),
        ))
    }
}
