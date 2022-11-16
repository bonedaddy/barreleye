use clickhouse::Client as ClickhouseClient;
use derive_more::Display;
use eyre::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{utils, Settings};

#[derive(Display, Debug, Serialize, Deserialize)]
pub enum Driver {
	#[display(fmt = "clickhouse")]
	#[serde(rename = "clickhouse")]
	Clickhouse,
}

pub struct Clickhouse {
	client: ClickhouseClient,
}

pub struct Warehouse {
	db_name: String,
	clickhouse: Clickhouse,
}

impl Warehouse {
	pub async fn new(settings: Arc<Settings>) -> Result<Self> {
		let (url_without_database, db_name) =
			utils::without_pathname(&settings.dsn.clickhouse);

		Ok(Self {
			db_name: db_name.clone(),
			clickhouse: Clickhouse {
				client: ClickhouseClient::default()
					.with_url(url_without_database)
					.with_database(db_name),
			},
		})
	}

	pub async fn run_migrations(self) -> Result<Self> {
		self.clickhouse
			.client
			.query(&format!(
				r#"CREATE DATABASE IF NOT EXISTS {};"#,
				self.db_name
			))
			.execute()
			.await?;

		self.clickhouse.client.query(&format!(
			r#"
			CREATE TABLE IF NOT EXISTS {}.transfers (
			  uuid UUID,
			  network_id UInt64,
			  block_height UInt64,
			  block_hash String,
			  tx_hash String,
			  from_address String,
			  to_address String,
			  asset_address String,
			  amount UInt256,
			  batch_amount UInt256,
			  created_at DateTime
			) ENGINE = ReplacingMergeTree
			ORDER BY (network_id, block_height, block_hash, tx_hash, from_address, to_address, asset_address, amount, batch_amount)
			PARTITION BY toYYYYMM(created_at);
			"#,
			self.db_name
		)).execute().await?;

		Ok(self)
	}

	pub fn get(&self) -> &ClickhouseClient {
		&self.clickhouse.client
	}
}
