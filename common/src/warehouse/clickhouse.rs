// @TODO all of this should be replaced by a proper driver
// right now all externally existing libs have bugs &| are out of date

use eyre::Result;
use std::{sync::Arc, time::Duration};

use crate::Settings;

pub struct Clickhouse {
	url: String,
	db_name: String,
}

impl Clickhouse {
	pub async fn new(settings: Arc<Settings>) -> Result<Self> {
		let url = settings.warehouse.clickhouse.url.clone();
		let db_name = settings.database.name.clone();

		let ch = Self { url, db_name };

		ch.run_migrations().await?;
		Ok(ch)
	}

	async fn run_migrations(&self) -> Result<()> {
		self.exec(&format!(
			r#"
			CREATE DATABASE IF NOT EXISTS {};
			"#,
			self.db_name
		))
		.await?;

		self.exec(&format!(
			r#"
            CREATE TABLE IF NOT EXISTS {}.transfers (
              network_id UInt64,
			  block UInt64,
              tx_hash String,
              from_address String,
              to_address String,
              token_address String,
              amount UInt256,
              created_at DateTime
            ) ENGINE = MergeTree()
            ORDER BY (network_id, created_at)
            PARTITION BY toYYYYMM(created_at);
			"#,
			self.db_name
		))
		.await?;

		Ok(())
	}

	async fn exec(&self, query: &str) -> Result<String> {
		let text = reqwest::Client::new()
			.post(self.url.clone())
			.body(query.to_string())
			.timeout(Duration::from_secs(5))
			.send()
			.await?
			.text()
			.await?;

		Ok(text)
	}
}
