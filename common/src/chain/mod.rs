use async_trait::async_trait;
use chrono::NaiveDateTime;
use derive_more::Display;
use eyre::Result;
use std::{collections::HashSet, ops::AddAssign, sync::Arc};

pub use crate::chain::bitcoin::Bitcoin;
use crate::{
	models::{Amount, Link, Network, Relation, Transfer},
	utils, BlockHeight, PrimaryId, RateLimiter, Warehouse,
};
pub use evm::Evm;
pub use u256::U256;

pub mod bitcoin;
pub mod evm;
pub mod u256;

pub type BoxedChain = Box<dyn ChainTrait>;

#[repr(u16)]
#[derive(Display, Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum ModuleId {
	BitcoinCoinbase = 101,
	BitcoinTransfer = 102,
	BitcoinBalance = 103,
	BitcoinRelationBalanceTransfer = 104,
	BitcoinRelationNoChange = 105,
	EvmTransfer = 201,
	EvmBalance = 202,
	EvmTokenTransfer = 203,
	EvmTokenBalance = 204,
}

#[async_trait]
pub trait ChainTrait: Send + Sync {
	async fn connect(&mut self) -> Result<bool>;
	fn is_connected(&self) -> bool;

	fn get_network(&self) -> Network;
	fn get_rpc(&self) -> Option<String>;
	fn get_module_ids(&self) -> Vec<ModuleId>;
	fn format_address(&self, address: &str) -> String;
	fn get_rate_limiter(&self) -> Option<Arc<RateLimiter>>;

	async fn get_block_height(&self) -> Result<BlockHeight>;

	async fn process_block(
		&self,
		block_height: BlockHeight,
		modules: Vec<ModuleId>,
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
	fn get_id(&self) -> ModuleId;
}

#[derive(Debug, Default, Clone)]
pub struct WarehouseData {
	saved_at: NaiveDateTime,
	pub transfers: HashSet<Transfer>,
	pub amounts: HashSet<Amount>,
	pub relations: HashSet<Relation>,
	pub links: HashSet<Link>,
}

impl WarehouseData {
	pub fn new() -> Self {
		Self { saved_at: utils::now(), ..Default::default() }
	}

	pub fn len(&self) -> usize {
		self.transfers.len() + self.amounts.len() + self.relations.len() + self.links.len()
	}

	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}

	pub fn should_commit(&self) -> bool {
		utils::ago_in_seconds(10) > self.saved_at && !self.is_empty() ||
			utils::ago_in_seconds(1) > self.saved_at && self.len() > 10_000
	}

	pub async fn commit(&mut self, warehouse: &Warehouse) -> Result<()> {
		if !self.transfers.is_empty() {
			Transfer::create_many(warehouse, self.transfers.clone().into_iter().collect()).await?;
		}

		if !self.amounts.is_empty() {
			Amount::create_many(warehouse, self.amounts.clone().into_iter().collect()).await?;
		}

		if !self.relations.is_empty() {
			Relation::create_many(warehouse, self.relations.clone().into_iter().collect()).await?;
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
		self.amounts.clear();
		self.relations.clear();
		self.links.clear();
	}
}

impl AddAssign for WarehouseData {
	fn add_assign(&mut self, rhs: WarehouseData) {
		self.transfers.extend(rhs.transfers);
		self.amounts.extend(rhs.amounts);
		self.relations.extend(rhs.relations);
		self.links.extend(rhs.links);
	}
}
