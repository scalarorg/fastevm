use reth_ethereum::{chainspec::ChainSpec, evm::EthEvmConfig, EthPrimitives};
use reth_node_api::{FullNodeTypes, NodeTypes};
use reth_node_builder::{components::ExecutorBuilder, BuilderContext};
// use reth_provider::LatestStateProviderRef;

use crate::executor::{ScalarEvmConfig, ScalarEvmFactory};

/// Builds a regular ethereum block executor that uses the custom EVM.
#[derive(Debug, Default, Clone, Copy)]
#[non_exhaustive]
pub struct ScalarExecutorBuilder;

impl<Node> ExecutorBuilder<Node> for ScalarExecutorBuilder
where
    Node: FullNodeTypes<Types: NodeTypes<ChainSpec = ChainSpec, Primitives = EthPrimitives>>,
{
    type EVM = ScalarEvmConfig<ChainSpec, ScalarEvmFactory>;

    async fn build_evm(self, ctx: &BuilderContext<Node>) -> eyre::Result<Self::EVM> {
        let evm_config =
            ScalarEvmConfig::new_with_evm_factory(ctx.chain_spec(), ScalarEvmFactory::default());
        Ok(evm_config)
    }
}
