use clickhouse::Client as ClickhouseClient;
use derive_more::Display;
use eyre::{Result, WrapErr};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{utils, Settings};

#[derive(Display, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum Driver {
	#[display(fmt = "Clickhouse")]
	#[serde(rename = "clickhouse")]
	Clickhouse,
}

pub struct Clickhouse {
	client: ClickhouseClient,
}

pub struct Warehouse {
	url_without_database: String,
	db_name: String,
	clickhouse: Clickhouse,
}

impl Warehouse {
	pub async fn new(settings: Arc<Settings>) -> Result<Self> {
		let (url_without_database, db_name) =
			utils::without_pathname(&settings.dsn.clickhouse);

		Ok(Self {
			url_without_database: url_without_database.clone(),
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
			.await
			.wrap_err(self.url_without_database.clone())?;

		self.clickhouse
			.client
			.query(&format!(
				r#"
			CREATE TABLE IF NOT EXISTS {}.transfers
			(
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
			)
			ENGINE = ReplacingMergeTree
			ORDER BY (
				network_id,
				block_height,
				block_hash,
				tx_hash,
				from_address,
				to_address,
				asset_address,
				amount,
				batch_amount
			)
			PARTITION BY toYYYYMM(created_at);
			"#,
				self.db_name
			))
			.execute()
			.await
			.wrap_err(self.url_without_database.clone())?;

		self.clickhouse
			.client
			.query(&format!(
				r#"
			CREATE MATERIALIZED VIEW IF NOT EXISTS {}.address_stats
			ENGINE = SummingMergeTree
			PARTITION BY network_id
			ORDER BY (address, network_id)
			POPULATE AS
			SELECT
			    a.address,
			    a.network_id,
			    a.in,
			    b.out
			FROM
			(
			    SELECT
			        to_address AS address,
			        network_id,
			        count(from_address) AS in
			    FROM {}.transfers
			    GROUP BY (network_id, to_address)
			) AS a
			LEFT JOIN
			(
			    SELECT
			        from_address AS address,
			        network_id,
			        count(to_address) AS out
			    FROM {}.transfers
			    GROUP BY (network_id, from_address)
			) AS b ON (a.address = b.address) AND (a.network_id = b.network_id)
			"#,
				self.db_name, self.db_name, self.db_name
			))
			.execute()
			.await
			.wrap_err(self.url_without_database.clone())?;

		Ok(self)
	}

	pub fn get(&self) -> &ClickhouseClient {
		&self.clickhouse.client
	}
}
