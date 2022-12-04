use async_trait::async_trait;
use ethers::types::Transaction;
use eyre::Result;

use crate::{Evm, IndexResults};
pub use transfer::EvmTransfer;

mod transfer;

#[async_trait]
pub trait EvmModuleTrait: Send + Sync {
	async fn run(
		&self,
		evm: &Evm,
		block_height: u64,
		block_time: u32,
		tx: Transaction,
	) -> Result<IndexResults>;
}
