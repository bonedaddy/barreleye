use async_trait::async_trait;
use eyre::Result;

pub use crate::bitcoin::Bitcoin;
use barreleye_common::models::Network;
pub use evm::Evm;
pub use networks::Networks;

mod bitcoin;
mod evm;
mod networks;

#[async_trait]
pub trait ChainTrait: Send + Sync {
	fn get_network(&self) -> Network;
	fn get_rpc(&self) -> Option<String>;
	async fn get_block_height(&self) -> Result<u64>;
	async fn get_last_processed_block(&self) -> Result<u64>;
	async fn process_blocks(&self, last_saved_block: u64) -> Result<u64>;
}
