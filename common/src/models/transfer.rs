use clickhouse::Row;
use eyre::Result;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
	chain::{u256, ModuleId, U256},
	models::PrimaryId,
	utils,
	warehouse::Warehouse,
};

static TABLE: &str = "transfers";

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
	pub asset_address: String,
	#[serde(with = "u256")]
	pub relative_amount: U256,
	#[serde(with = "u256")]
	pub batch_amount: U256,
	pub created_at: u32,
}

pub use Model as Transfer;

impl Model {
	pub fn new(
		module_id: ModuleId,
		network_id: PrimaryId,
		block_height: u64,
		tx_hash: String,
		from_address: String,
		to_address: String,
		asset_address: Option<String>,
		relative_amount: U256,
		batch_amount: U256,
		created_at: u32,
	) -> Self {
		Self {
			uuid: utils::new_uuid(),
			module_id: module_id as u16,
			network_id: network_id as u64,
			block_height,
			tx_hash,
			from_address,
			to_address,
			asset_address: asset_address.unwrap_or_default(),
			relative_amount,
			batch_amount,
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
