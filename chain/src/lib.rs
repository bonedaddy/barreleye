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
	async fn process_blocks(&self) -> Result<()>;
}

#[derive(Debug)]
pub struct IndexTransferV1 {
	tx_hash: String,
	from_address: String,
	to_address: String,
	amount: String,
	batch_amount: String,
}
