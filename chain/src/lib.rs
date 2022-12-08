use async_trait::async_trait;
use chrono::NaiveDateTime;
use eyre::Result;
use governor::{
	clock::DefaultClock,
	state::{direct::NotKeyed, InMemoryState},
	RateLimiter as GovernorRateLimiter,
};
use std::{borrow::BorrowMut, collections::HashSet, ops::AddAssign};
use tokio::sync::{mpsc::Sender, oneshot::Receiver};

pub use crate::bitcoin::Bitcoin;
use barreleye_common::{
	models::{Link, Network, PrimaryId, Transfer},
	utils, BlockHeight, ChainModuleId, Warehouse,
};
pub use evm::Evm;
pub use networks::Networks;

mod bitcoin;
mod evm;
mod networks;

pub type RateLimiter = GovernorRateLimiter<NotKeyed, InMemoryState, DefaultClock>;

pub struct CanExit {
	network_id: PrimaryId,
	module_id: Option<ChainModuleId>,
	notified: bool,
	done: Sender<(PrimaryId, Option<ChainModuleId>)>,
	receipt: Receiver<()>,
}

impl CanExit {
	pub fn new(
		network_id: PrimaryId,
		module_id: Option<ChainModuleId>,
		done: Sender<(PrimaryId, Option<ChainModuleId>)>,
		receipt: Receiver<()>,
	) -> Self {
		Self { network_id, module_id, notified: false, done, receipt }
	}

	pub async fn notify(&mut self) -> Result<()> {
		if !self.notified {
			self.done.send((self.network_id, self.module_id)).await?;
			self.notified = self.receipt.borrow_mut().await.is_ok();
		}

		Ok(())
	}
}

#[async_trait]
pub trait ChainTrait: Send + Sync {
	fn get_network(&self) -> Network;
	fn get_rpc(&self) -> Option<String>;
	fn get_module_ids(&self) -> Vec<ChainModuleId>;
	async fn get_block_height(&self) -> Result<BlockHeight>;
	async fn get_last_processed_block(&self) -> Result<BlockHeight>;
	async fn process_block(
		&self,
		block_height: BlockHeight,
		modules: Vec<ChainModuleId>,
	) -> Result<Option<WarehouseData>>;
}

#[async_trait]
pub trait ModuleTrait {
	fn new(network_id: PrimaryId) -> Self
	where
		Self: Sized;
	fn get_id(&self) -> ChainModuleId;
}

#[derive(Debug, Default)]
pub struct WarehouseData {
	saved_at: NaiveDateTime,
	transfers: HashSet<Transfer>,
	links: HashSet<Link>,
}

impl WarehouseData {
	pub fn new() -> Self {
		Self { saved_at: utils::now(), ..Default::default() }
	}

	pub fn len(&self) -> usize {
		self.transfers.len() + self.links.len()
	}

	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}

	pub fn should_commit(&self) -> bool {
		utils::ago_in_seconds(5) > self.saved_at && self.len() > 1_000
	}

	pub async fn commit(&mut self, warehouse: &Warehouse) -> Result<()> {
		if !self.transfers.is_empty() {
			Transfer::create_many(warehouse, self.transfers.clone().into_iter().collect()).await?;
		}

		if !self.links.is_empty() {
			Link::create_many(warehouse, self.links.clone().into_iter().collect()).await?;
		}

		self.reset();
		Ok(())
	}

	pub fn reset(&mut self) {
		self.saved_at = utils::now();

		self.transfers.clear();
		self.links.clear();
	}
}

impl AddAssign for WarehouseData {
	fn add_assign(&mut self, rhs: WarehouseData) {
		self.transfers.extend(rhs.transfers);
		self.links.extend(rhs.links);
	}
}
