use async_trait::async_trait;
use chrono::NaiveDateTime;
use eyre::Result;
use std::{collections::HashSet, ops::AddAssign, sync::Arc};

pub use crate::chain::bitcoin::Bitcoin;
use crate::{
	models::{Link, Network, Transfer, TxAmount},
	utils, BlockHeight, ChainModuleId, PrimaryId, RateLimiter, Warehouse,
};
pub use evm::Evm;

pub mod bitcoin;
pub mod evm;

pub type BoxedChain = Box<dyn ChainTrait>;

#[async_trait]
pub trait ChainTrait: Send + Sync {
	async fn connect(&mut self) -> Result<bool>;
	fn is_connected(&self) -> bool;

	fn get_network(&self) -> Network;
	fn get_rpc(&self) -> Option<String>;
	fn get_module_ids(&self) -> Vec<ChainModuleId>;
	fn format_address(&self, address: &str) -> String;
	fn get_rate_limiter(&self) -> Option<Arc<RateLimiter>>;

	async fn get_block_height(&self) -> Result<BlockHeight>;

	async fn process_block(
		&self,
		block_height: BlockHeight,
		modules: Vec<ChainModuleId>,
	) -> Result<Option<WarehouseData>>;

	async fn rate_limit(&self) {
		if let Some(rate_limiter) = &self.get_rate_limiter() {
			rate_limiter.until_ready().await;
		}
	}
}

#[async_trait]
pub trait ModuleTrait {
	fn new(network_id: PrimaryId) -> Self
	where
		Self: Sized;
	fn get_id(&self) -> ChainModuleId;
}

#[derive(Debug, Default, Clone)]
pub struct WarehouseData {
	saved_at: NaiveDateTime,
	transfers: HashSet<Transfer>,
	tx_amounts: HashSet<TxAmount>,
	links: HashSet<Link>,
}

impl WarehouseData {
	pub fn new() -> Self {
		Self { saved_at: utils::now(), ..Default::default() }
	}

	pub fn len(&self) -> usize {
		self.transfers.len() + self.tx_amounts.len() + self.links.len()
	}

	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}

	pub fn should_commit(&self) -> bool {
		utils::ago_in_seconds(5) > self.saved_at && self.len() > 25_000
	}

	pub async fn commit(&mut self, warehouse: &Warehouse) -> Result<()> {
		if !self.transfers.is_empty() {
			Transfer::create_many(warehouse, self.transfers.clone().into_iter().collect()).await?;
		}

		if !self.tx_amounts.is_empty() {
			TxAmount::create_many(warehouse, self.tx_amounts.clone().into_iter().collect()).await?;
		}

		if !self.links.is_empty() {
			Link::create_many(warehouse, self.links.clone().into_iter().collect()).await?;
		}

		self.clear();

		Ok(())
	}

	pub fn clear(&mut self) {
		self.saved_at = utils::now();

		self.transfers.clear();
		self.tx_amounts.clear();
		self.links.clear();
	}
}

impl AddAssign for WarehouseData {
	fn add_assign(&mut self, rhs: WarehouseData) {
		self.transfers.extend(rhs.transfers);
		self.tx_amounts.extend(rhs.tx_amounts);
		self.links.extend(rhs.links);
	}
}
