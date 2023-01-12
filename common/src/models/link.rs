use clickhouse::Row;
use eyre::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::{
	models::{transfer::TABLE as TRANSFERS_TABLE, PrimaryId},
	warehouse::Warehouse,
	BlockHeight,
};

pub static TABLE: &str = "links";

#[derive(PartialEq, Eq, Hash, Debug, Clone, Row, Serialize, Deserialize)]
pub struct Model {
	pub network_id: u64,
	pub block_height: u64,
	pub from_address: String,
	pub to_address: String,
	// @TODO this should be `Uuid` but the current Clickhouse driver does not support Vec<Uuid> atm
	// #[serde(with = "clickhouse::serde::uuid")]
	pub transfer_uuids: Vec<String>,
	pub created_at: u32,
}

pub use Model as Link;

impl Model {
	pub fn new(
		network_id: PrimaryId,
		block_height: u64,
		from_address: &str,
		to_address: &str,
		transfer_uuids: Vec<String>,
		created_at: u32,
	) -> Self {
		Self {
			network_id: network_id as u64,
			block_height,
			from_address: from_address.to_string(),
			to_address: to_address.to_string(),
			transfer_uuids,
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

	pub async fn get_all_by_source(
		warehouse: &Warehouse,
		network_id: PrimaryId,
		address: &str,
	) -> Result<Vec<Self>> {
		Ok(warehouse
			.get()
			.query(&format!(
				r#"
					SELECT *
					FROM {TABLE}
					WHERE network_id = ? AND from_address = ?
                "#
			))
			.bind(network_id)
			.bind(address.to_string())
			.fetch_all::<Model>()
			.await?)
	}

	pub async fn get_all_for_seed_blocks(
		warehouse: &Warehouse,
		network_id: PrimaryId,
		(block_height_min, block_height_max): (BlockHeight, BlockHeight),
	) -> Result<Vec<Self>> {
		Ok(warehouse
			.get()
			.query(&format!(
				r#"
					SELECT *
					FROM {TABLE}
					WHERE network_id = ? AND to_address IN (
					    SELECT from_address
					    FROM {TRANSFERS_TABLE}
					    WHERE
							network_id = ? AND
							length(from_address) > 0 AND
							length(to_address) > 0 AND
							block_height >= ? AND
							block_height <= ?
					)
                "#
			))
			.bind(network_id)
			.bind(network_id)
			.bind(block_height_min)
			.bind(block_height_max)
			.fetch_all::<Model>()
			.await?)
	}

	pub async fn delete_all_by_sources(
		warehouse: &Warehouse,
		sources: HashMap<PrimaryId, HashSet<String>>,
	) -> Result<()> {
		if !sources.is_empty() {
			let filter = sources
				.into_iter()
				.map(|(network_id, addresses)| {
					let escaped_addresses = addresses
						.into_iter()
						.map(|a| format!("'{}'", a.replace('\\', "\\\\").replace('\'', "\\'")))
						.collect::<Vec<String>>()
						.join(",");

					format!(
						"(network_id = {} AND from_address IN ({}))",
						network_id, escaped_addresses
					)
				})
				.collect::<Vec<String>>()
				.join(" OR ");

			warehouse
				.get()
				.query(&format!("ALTER TABLE {TABLE} DELETE WHERE {filter}"))
				.execute()
				.await?;
		}

		Ok(())
	}
}
