use async_trait::async_trait;
use chrono::NaiveDateTime;
use eyre::Result;
use std::{
	borrow::BorrowMut,
	collections::HashSet,
	ops::AddAssign,
	sync::{atomic::AtomicBool, Arc},
};
use tokio::sync::{mpsc::Sender, oneshot::Receiver};

pub use crate::bitcoin::Bitcoin;
use barreleye_common::{
	models::{Link, Network, PrimaryId, Transfer},
	utils, ChainModuleId, Warehouse,
};
pub use evm::Evm;
pub use networks::Networks;

mod bitcoin;
mod evm;
mod networks;

pub struct CanExit {
	network_id: PrimaryId,
	notified: bool,
	done: Sender<PrimaryId>,
	receipt: Receiver<()>,
}

impl CanExit {
	pub fn new(
		network_id: PrimaryId,
		done: Sender<PrimaryId>,
		receipt: Receiver<()>,
	) -> Self {
		Self { network_id, notified: false, done, receipt }
	}

	pub async fn notify(&mut self) -> Result<()> {
		if !self.notified {
			self.done.send(self.network_id).await?;
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
	async fn get_block_height(&self) -> Result<u64>;
	async fn get_last_processed_block(&self) -> Result<u64>;
	async fn process_blocks(
		&self,
		starting_block: u64,
		ending_block: Option<u64>,
		modules: Vec<ChainModuleId>,
		should_keep_going: Arc<AtomicBool>,
		can_exit: CanExit,
	) -> Result<(u64, IndexResults)>;
	async fn process_block(
		&self,
		block_height: u64,
		modules: Vec<ChainModuleId>,
	) -> Result<Option<IndexResults>>;
}

#[async_trait]
pub trait ModuleTrait {
	fn new(network_id: PrimaryId) -> Self
	where
		Self: Sized;
	fn get_id(&self) -> ChainModuleId;
}

#[derive(Debug, Default)]
pub struct IndexResults {
	saved_at: NaiveDateTime,
	transfers: HashSet<Transfer>,
	links: HashSet<Link>,
}

impl IndexResults {
	pub fn new() -> Self {
		Self { saved_at: utils::now(), ..Default::default() }
	}

	pub fn len(&self) -> usize {
		self.transfers.len() + self.links.len()
	}

	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}

	pub fn is_ready_to_commit(&self) -> bool {
		utils::ago_in_seconds(5) > self.saved_at && self.len() > 5_000
	}

	pub async fn commit(&mut self, warehouse: &Warehouse) -> Result<()> {
		if !self.transfers.is_empty() {
			Transfer::create_many(
				warehouse,
				self.transfers.clone().into_iter().collect(),
			)
			.await?;
		}

		if !self.links.is_empty() {
			Link::create_many(
				warehouse,
				self.links.clone().into_iter().collect(),
			)
			.await?;
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

impl AddAssign for IndexResults {
	fn add_assign(&mut self, rhs: IndexResults) {
		self.transfers.extend(rhs.transfers);
		self.links.extend(rhs.links);
	}
}
