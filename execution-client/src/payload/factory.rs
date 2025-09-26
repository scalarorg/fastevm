//! Payload component configuration for the Ethereum node.

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

use crate::{consensus::ConsensusPool, payload::MysticetiPayloadBuilder};
use reth_ethereum_payload_builder::EthereumBuilderConfig;
use reth_payload_builder::{EthBuiltPayload, EthPayloadBuilderAttributes};
use std::sync::Arc;
// use reth_transaction_pool::{PoolTransaction, TransactionPool};

#[non_exhaustive]
pub struct MysticetiPayloadBuilderFactory<Pool: TransactionPool>
where
    Pool: TransactionPool,
{
    consensus_pool: Arc<ConsensusPool<Pool>>,
}

impl<Pool: TransactionPool> MysticetiPayloadBuilderFactory<Pool>
where
    Pool: TransactionPool,
{
    pub fn new(consensus_pool: Arc<ConsensusPool<Pool>>) -> Self {
        Self { consensus_pool }
    }
}

impl<Types, Node, Pool, Evm> PayloadBuilderBuilder<Node, Pool, Evm>
    for MysticetiPayloadBuilderFactory<Pool>
where
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

        // Ok(reth_ethereum_payload_builder::EthereumPayloadBuilder::new(
        //     ctx.provider().clone(),
        //     pool,
        //     evm_config,
        //     EthereumBuilderConfig::new().with_gas_limit(gas_limit),
        // ))
        let MysticetiPayloadBuilderFactory { consensus_pool } = self;
        Ok(MysticetiPayloadBuilder::new(
            ctx.provider().clone(),
            pool,
            consensus_pool,
            evm_config,
            EthereumBuilderConfig::new().with_gas_limit(gas_limit),
        ))
    }
}
