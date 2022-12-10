use clickhouse::Row;
use eyre::Result;
use primitive_types::U256;
use serde::{Deserialize, Serialize};

use crate::{u256, warehouse::Warehouse};

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
		// @TODO until I256 is implemented, doing the "group by" statement
		// "SELECT ?fields FROM {TABLE} WHERE address = ?"

		Ok(warehouse
			.get()
			.query(&format!(
				r#"
                    SELECT
                        network_id,
                        address,
                        asset_address,
                        SUM(amount)
                    FROM {TABLE}
                    WHERE address = ?
                    GROUP BY (network_id, address, asset_address)
                "#
			))
			.bind(address)
			.fetch_all::<Model>()
			.await?)
	}
}
