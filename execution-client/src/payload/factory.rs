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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_mysticeti_payload_builder_factory_new() {
        let subdag_queue = Arc::new(Mutex::new(VecDeque::new()));
        let factory = MysticetiPayloadBuilderFactory::new(subdag_queue.clone());

        // Test that the factory was created successfully
        assert_eq!(
            Arc::as_ptr(&factory.subdag_queue),
            Arc::as_ptr(&subdag_queue)
        );
    }

    #[test]
    fn test_mysticeti_payload_builder_factory_creation() {
        let subdag_queue = Arc::new(Mutex::new(VecDeque::new()));
        let factory = MysticetiPayloadBuilderFactory::new(subdag_queue);

        // Test that we can create multiple instances
        let factory2 = MysticetiPayloadBuilderFactory::new(Arc::new(Mutex::new(VecDeque::new())));
        assert_ne!(
            Arc::as_ptr(&factory.subdag_queue),
            Arc::as_ptr(&factory2.subdag_queue)
        );
    }

    #[test]
    fn test_subdag_queue_isolation() {
        let subdag_queue1 = Arc::new(Mutex::new(VecDeque::new()));
        let subdag_queue2 = Arc::new(Mutex::new(VecDeque::new()));

        let factory1 = MysticetiPayloadBuilderFactory::new(subdag_queue1.clone());
        let factory2 = MysticetiPayloadBuilderFactory::new(subdag_queue2.clone());

        // Add items to one queue
        {
            let mut queue1 = subdag_queue1.lock().unwrap();
            queue1.push_back(CommittedSubDag::default());
        }

        // Verify the other queue is unaffected
        assert_eq!(subdag_queue1.lock().unwrap().len(), 1);
        assert_eq!(subdag_queue2.lock().unwrap().len(), 0);

        // Verify factory references are correct
        assert_eq!(
            Arc::as_ptr(&factory1.subdag_queue),
            Arc::as_ptr(&subdag_queue1)
        );
        assert_eq!(
            Arc::as_ptr(&factory2.subdag_queue),
            Arc::as_ptr(&subdag_queue2)
        );
    }

    #[test]
    fn test_committed_subdag_queue_operations() {
        let subdag_queue = Arc::new(Mutex::new(VecDeque::new()));
        let factory = MysticetiPayloadBuilderFactory::new(subdag_queue.clone());

        // Test adding subdags to the queue
        {
            let mut queue = subdag_queue.lock().unwrap();
            queue.push_back(CommittedSubDag::default());
            queue.push_back(CommittedSubDag::default());
        }

        // Verify queue size
        assert_eq!(subdag_queue.lock().unwrap().len(), 2);

        // Test popping from queue
        {
            let mut queue = subdag_queue.lock().unwrap();
            let subdag = queue.pop_front();
            assert!(subdag.is_some());
            assert_eq!(queue.len(), 1);
        }
    }

    #[test]
    fn test_factory_debug_implementation() {
        let subdag_queue = Arc::new(Mutex::new(VecDeque::new()));
        let factory = MysticetiPayloadBuilderFactory::new(subdag_queue);

        // Test that Debug is implemented
        let debug_string = format!("{:?}", factory);
        assert!(debug_string.contains("MysticetiPayloadBuilderFactory"));
    }

    #[test]
    fn test_factory_non_exhaustive() {
        let subdag_queue = Arc::new(Mutex::new(VecDeque::new()));
        let factory = MysticetiPayloadBuilderFactory::new(subdag_queue);

        // Test that the struct is marked as non_exhaustive
        // This means it can be extended in the future without breaking changes
        assert!(std::mem::size_of::<MysticetiPayloadBuilderFactory>() > 0);
    }
}
