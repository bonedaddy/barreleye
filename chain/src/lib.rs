use async_trait::async_trait;
use eyre::Result;
use std::sync::Arc;

use barreleye_common::{models::Network, Db};

mod bitcoin;
mod evm;
mod solana;

pub use bitcoin::Bitcoin;
pub use evm::Evm;
pub use solana::Solana;

pub mod networks;
pub use networks::Networks;

#[async_trait]
pub trait ChainTrait: Send + Sync {
	fn get_network(&self) -> Network;

	fn get_rpc(&self) -> Option<String>;

	async fn watch(&self, db: Arc<Db>) -> Result<()>;
}
