use alloy_consensus::Block;
use alloy_evm::block::{BlockExecutionError, BlockExecutorFactory};
use alloy_primitives::Bytes;
use reth_chainspec::{EthChainSpec, EthereumHardforks};
use reth_ethereum::{
    evm::{
        primitives::execute::{BlockAssembler, BlockAssemblerInput},
        EthBlockAssembler,
    },
    primitives::Receipt,
    TransactionSigned,
};
use reth_evm::eth::EthBlockExecutionCtx;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct ScalarBlockAssembler<ChainSpec = reth_chainspec::ChainSpec> {
    block_assembler: EthBlockAssembler<ChainSpec>,
}

impl<ChainSpec> ScalarBlockAssembler<ChainSpec> {
    pub fn new(chain_spec: Arc<ChainSpec>) -> Self {
        Self {
            block_assembler: EthBlockAssembler::new(chain_spec),
        }
    }
    pub fn extra_data(&self) -> Bytes {
        self.block_assembler.extra_data.clone()
    }
    pub fn with_extra_data(mut self, extra_data: Bytes) -> Self {
        self.block_assembler.extra_data = extra_data;
        self
    }
}

impl<F, ChainSpec> BlockAssembler<F> for ScalarBlockAssembler<ChainSpec>
where
    F: for<'a> BlockExecutorFactory<
        ExecutionCtx<'a> = EthBlockExecutionCtx<'a>,
        Transaction = TransactionSigned,
        Receipt: Receipt,
    >,
    ChainSpec: EthChainSpec + EthereumHardforks,
{
    type Block = Block<F::Transaction>;

    fn assemble_block(
        &self,
        input: BlockAssemblerInput<'_, '_, F>,
    ) -> Result<Self::Block, BlockExecutionError> {
        Ok(self
            .block_assembler
            .assemble_block(input)?
            .map_header(From::from))
    }
}
