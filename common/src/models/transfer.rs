use clickhouse::Row;
use eyre::Result;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{models::PrimaryId, utils, warehouse::Warehouse, Address};

static TABLE: &str = "transfers";

#[derive(Debug, Row, Serialize, Deserialize)]
pub struct Model {
	#[serde(with = "clickhouse::uuid")]
	pub uuid: Uuid,
	pub network_id: u64,
	pub block_height: u64,
	pub block_hash: String,
	pub tx_hash: String,
	pub from_address: String,
	pub to_address: String,
	pub asset_address: String,
	pub amount: String,       // @TODO no support for U256 yet
	pub batch_amount: String, // @TODO no support for U256 yet
	pub created_at: u32,
}

pub use Model as Transfer;

impl Model {
	pub fn new(
		network_id: PrimaryId,
		block_height: u64,
		block_hash: String,
		tx_hash: String,
		from_address: Address,
		to_address: Address,
		asset_address: Option<Address>,
		amount: String,
		batch_amount: String,
	) -> Self {
		Self {
			uuid: utils::new_uuid(),
			network_id: network_id as u64,
			block_height,
			block_hash: block_hash.to_lowercase(),
			tx_hash: tx_hash.to_lowercase(),
			from_address: from_address.to_string().to_lowercase(),
			to_address: to_address.to_string().to_lowercase(),
			asset_address: asset_address
				.unwrap_or_else(Address::blank)
				.to_string()
				.to_lowercase(),
			amount,
			batch_amount,
			created_at: utils::now().timestamp() as u32,
		}
	}

	pub async fn create_many(
		warehouse: &Warehouse,
		models: Vec<Self>,
	) -> Result<()> {
		let mut insert = warehouse.get().insert(TABLE)?;
		for model in models.into_iter() {
			insert.write(&model).await?;
		}

		Ok(insert.end().await?)
	}

	pub async fn get_block_height(
		warehouse: &Warehouse,
		network_id: PrimaryId,
	) -> Result<Option<u64>> {
		let record = warehouse.get()
		    .query(&format!("SELECT ?fields FROM {TABLE} WHERE network_id = ? ORDER BY block_height DESC"))
		    .bind(network_id)
		    .fetch_one::<Model>().await;

		Ok(match record {
			Ok(row) => Some(row.block_height),
			_ => None,
		})
	}
}
