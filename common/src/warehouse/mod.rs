use clickhouse::Client as ClickhouseClient;
use eyre::Result;
use std::sync::Arc;

use crate::Settings;

pub struct Clickhouse {
	client: ClickhouseClient,
}

pub struct Warehouse {
	db_name: String,
	clickhouse: Clickhouse,
}

impl Warehouse {
	pub async fn new(settings: Arc<Settings>) -> Result<Self> {
		Ok(Self {
			db_name: settings.warehouse.name.clone(),
			clickhouse: Clickhouse {
				client: ClickhouseClient::default()
					.with_url(settings.warehouse.clickhouse.url.clone())
					.with_database(settings.warehouse.name.clone()),
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
			  amount String,
			  batch_amount String,
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
