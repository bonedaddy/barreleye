use clickhouse::Row;
use eyre::Result;
use serde::{Deserialize, Serialize};

use crate::{
	chain::{u256, U256},
	warehouse::Warehouse,
};

static TABLE: &str = "amounts";

#[derive(PartialEq, Eq, Hash, Debug, Clone, Row, Serialize, Deserialize)]
pub struct Model {
	pub network_id: u64,
	pub address: String,
	pub asset_address: String,
	#[serde(with = "u256")]
	pub amount: U256,
}

pub use Model as Amount;

impl Model {
	pub async fn get_all_by_address(warehouse: &Warehouse, address: &str) -> Result<Vec<Model>> {
		// @TODO until I256 is implemented, doing this hacky "group by" statement
		// ideally: "SELECT ?fields FROM {TABLE} WHERE address = ?"

		Ok(warehouse
			.get()
			.query(&format!(
				r#"
					SELECT *
					FROM (
	                    SELECT
	                        network_id,
	                        address,
	                        asset_address,
	                        SUM(amount) as amount
	                    FROM {TABLE}
	                    WHERE address = ?
	                    GROUP BY (network_id, address, asset_address)
					)
					WHERE amount >= 0
                "#
			))
			.bind(address)
			.fetch_all::<Model>()
			.await?)
	}
}
