use async_trait::async_trait;
use ethers::types::{Transaction, TransactionReceipt};
use eyre::Result;

use crate::{
	chain::{Evm, ModuleTrait, WarehouseData},
	BlockHeight,
};
pub use balance::EvmBalance;
pub use token_balance::EvmTokenBalance;
pub use token_transfer::EvmTokenTransfer;
pub use transfer::EvmTransfer;

mod balance;
mod token_balance;
mod token_transfer;
mod transfer;

#[async_trait]
pub trait EvmModuleTrait: ModuleTrait + Send + Sync {
	async fn run(
		&self,
		evm: &Evm,
		block_height: BlockHeight,
		block_time: u32,
		tx: Transaction,
		receipt: TransactionReceipt,
	) -> Result<WarehouseData>;
}
