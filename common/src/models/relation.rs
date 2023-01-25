use clickhouse::Row;
use eyre::Result;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
	chain::ModuleId,
	models::{PrimaryId, PrimaryIds},
	utils,
	warehouse::Warehouse,
};

pub static TABLE: &str = "experimental_relations";

#[repr(u16)]
pub enum Reason {
	WholeBalanceTransfer = 1,
	NoChangeInUtxo = 2,
}

#[derive(PartialEq, Eq, Hash, Debug, Clone, Row, Serialize, Deserialize)]
pub struct Model {
	#[serde(with = "clickhouse::serde::uuid")]
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

pub use Model as Relation;

impl Model {
	pub fn new(
		module_id: ModuleId,
		network_id: PrimaryId,
		block_height: u64,
		tx_hash: &str,
		from_address: &str,
		to_address: &str,
		reason: Reason,
		created_at: u32,
	) -> Self {
		Self {
			uuid: utils::new_uuid(),
			module_id: module_id as u16,
			network_id: network_id as u64,
			block_height,
			tx_hash: tx_hash.to_string(),
			from_address: from_address.to_string(),
			to_address: to_address.to_string(),
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

	pub async fn delete_all_by_network_id(
		warehouse: &Warehouse,
		network_ids: PrimaryIds,
	) -> Result<()> {
		Ok(warehouse
			.get()
			.query(&format!(
				r#"
					ALTER {TABLE}
					DELETE WHERE network_id IN ?
                "#
			))
			.bind(network_ids.into_iter().collect::<Vec<PrimaryId>>())
			.execute()
			.await?)
	}
}
