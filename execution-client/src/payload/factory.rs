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

use reth_ethereum_payload_builder::EthereumBuilderConfig;
use reth_extension::CommittedSubDag;
//use reth_ethereum_payload_builder::EthereumBuilderConfig;
use crate::payload::MysticetiPayloadBuilder;
use reth_payload_builder::{EthBuiltPayload, EthPayloadBuilderAttributes};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};
// use reth_transaction_pool::{PoolTransaction, TransactionPool};

#[derive(Debug)]
#[non_exhaustive]
pub struct MysticetiPayloadBuilderFactory {
    subdag_queue: Arc<Mutex<VecDeque<CommittedSubDag>>>,
}

impl MysticetiPayloadBuilderFactory {
    pub fn new(subdag_queue: Arc<Mutex<VecDeque<CommittedSubDag>>>) -> Self {
        Self { subdag_queue }
    }
}

impl<Types, Node, Pool, Evm> PayloadBuilderBuilder<Node, Pool, Evm>
    for MysticetiPayloadBuilderFactory
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
        let MysticetiPayloadBuilderFactory { subdag_queue } = self;
        Ok(MysticetiPayloadBuilder::new(
            ctx.provider().clone(),
            pool,
            subdag_queue,
            evm_config,
            EthereumBuilderConfig::new().with_gas_limit(gas_limit),
        ))
    }
}
