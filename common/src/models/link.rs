use clickhouse::Row;
use eyre::Result;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{models::PrimaryId, utils, warehouse::Warehouse, Address, ChainModuleId};

static TABLE: &str = "experimental_links";

#[repr(u16)]
pub enum Reason {
	PossibleSelfTransfer = 1,
}

#[derive(PartialEq, Eq, Hash, Debug, Clone, Row, Serialize, Deserialize)]
pub struct Model {
	#[serde(with = "clickhouse::uuid")]
	pub uuid: Uuid,
	pub module_id: u16,
	pub network_id: u64,
	pub block_height: u64,
	pub tx_hash: String,
	pub from_address: String,
	pub to_address: String,
	pub reason: u16,
	pub created_at: u32,
}

pub use Model as Link;

impl Model {
	pub fn new(
		module_id: ChainModuleId,
		network_id: PrimaryId,
		block_height: u64,
		tx_hash: String,
		from_address: Address,
		to_address: Address,
		reason: Reason,
		created_at: u32,
	) -> Self {
		Self {
			uuid: utils::new_uuid(),
			module_id: module_id as u16,
			network_id: network_id as u64,
			block_height,
			tx_hash: tx_hash.to_lowercase(),
			from_address: from_address.to_string().to_lowercase(),
			to_address: to_address.to_string().to_lowercase(),
			reason: reason as u16,
			created_at,
		}
	}

	pub async fn create_many(warehouse: &Warehouse, models: Vec<Self>) -> Result<()> {
		let mut insert = warehouse.get().insert(TABLE)?;
		for model in models.into_iter() {
			insert.write(&model).await?;
		}

		Ok(insert.end().await?)
	}
}
