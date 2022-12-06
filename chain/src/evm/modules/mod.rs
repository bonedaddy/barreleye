use async_trait::async_trait;
use ethers::types::Transaction;
use eyre::Result;

use crate::{Evm, ModuleTrait, WarehouseData};
use barreleye_common::BlockHeight;
pub use transfer::EvmTransfer;

mod transfer;

#[async_trait]
pub trait EvmModuleTrait: ModuleTrait + Send + Sync {
	async fn run(
		&self,
		evm: &Evm,
		block_height: BlockHeight,
		block_time: u32,
		tx: Transaction,
	) -> Result<WarehouseData>;
}
