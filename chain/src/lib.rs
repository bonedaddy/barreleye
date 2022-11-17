use async_trait::async_trait;
use eyre::Result;
use std::sync::{atomic::AtomicBool, Arc};
use tokio::sync::mpsc::{Receiver, Sender};

pub use crate::bitcoin::Bitcoin;
use barreleye_common::models::{Network, PrimaryId};
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
	async fn process_blocks(
		&self,
		last_saved_block: u64,
		should_keep_going: Arc<AtomicBool>,
		i_am_done: Sender<PrimaryId>,
		receipt: Receiver<()>,
	) -> Result<u64>;
}
