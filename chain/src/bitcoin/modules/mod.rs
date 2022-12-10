use async_trait::async_trait;
use bitcoin::blockdata::transaction::Transaction;
use eyre::Result;
use std::collections::HashMap;

use crate::{Bitcoin, ModuleTrait, WarehouseData};
use barreleye_common::BlockHeight;
pub use coinbase::BitcoinCoinbase;
pub use link::BitcoinLink;
pub use transfer::BitcoinTransfer;
pub use tx_amount::BitcoinTxAmount;

mod coinbase;
mod link;
mod transfer;
mod tx_amount;

#[async_trait]
pub trait BitcoinModuleTrait: ModuleTrait + Send + Sync {
	async fn run(
		&self,
		bitcoin: &Bitcoin,
		block_height: BlockHeight,
		block_time: u32,
		tx: Transaction,
		inputs: HashMap<String, u64>,
		outputs: HashMap<String, u64>,
	) -> Result<WarehouseData>;
}
