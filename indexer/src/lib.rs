use console::style;
use derive_more::Display;
use eyre::Result;
use num_format::{SystemLocale, ToFormattedString};
use serde_json::Value as JsonValue;
use std::sync::Arc;
use tokio::{
	signal,
	sync::{
		broadcast,
		mpsc::{self, Sender},
	},
	time::{sleep, Duration},
};
use uuid::Uuid;

use barreleye_common::{
	chain::{ModuleId, WarehouseData},
	models::{Config, ConfigKey, PrimaryId},
	quit, utils, App, AppError, BlockHeight, Progress, ProgressReadyType, ProgressStep, Verbosity,
	Warnings,
};

mod blocks;
mod upstream;

#[derive(Clone, Debug)]
struct NetworkParams {
	pub network_id: PrimaryId,
	pub range: (BlockHeight, Option<BlockHeight>),
	pub modules: Vec<ModuleId>,
}

impl NetworkParams {
	pub fn new(
		network_id: PrimaryId,
		min: BlockHeight,
		max: Option<BlockHeight>,
		modules: &[ModuleId],
	) -> Self {
		Self { network_id, range: (min, max), modules: modules.to_vec() }
	}
}

#[derive(Display, Debug)]
enum IndexType {
	#[display(fmt = "blocks")]
	Blocks,
	#[display(fmt = "upstream")]
	Upstream,
}

pub struct Indexer {
	app: Arc<App>,
}

impl Indexer {
	pub fn new(app: Arc<App>) -> Self {
		Self { app }
	}

	pub async fn start(&self, warnings: Warnings, progress: Progress) -> Result<()> {
		let verbose = self.app.verbosity as u8 > Verbosity::Silent as u8;

		if self.app.is_indexer && !self.app.is_server {
			progress.show(ProgressStep::Ready(ProgressReadyType::Indexer, warnings));
		}

		let ret = tokio::select! {
			_ = signal::ctrl_c() => Ok(()),
			v = self.start_primary_check() => v,
			v = self.index_blocks(verbose) => v,
			v = self.index_upstream(verbose) => v,
		};

		if ret.is_err() {
			quit(AppError::IndexingFailed { error: ret.as_ref().unwrap_err().to_string() });
		}

		ret
	}

	async fn start_primary_check(&self) -> Result<()> {
		let primary_promotion = self.app.settings.primary_promotion;
		let db = self.app.db();
		let uuid = self.app.uuid;

		loop {
			let cool_down_period = utils::ago_in_seconds(primary_promotion / 2);

			let last_primary = Config::get::<_, Uuid>(db, ConfigKey::Primary).await?;
			match last_primary {
				None => {
					// first run ever
					Config::set::<_, Uuid>(db, ConfigKey::Primary, uuid).await?;
				}
				Some(hit) if hit.value == uuid && hit.updated_at >= cool_down_period => {
					// if primary, check-in only if cool-down period has not started yet ↑
					if Config::set_where::<_, Uuid>(db, ConfigKey::Primary, uuid, hit).await? {
						self.app.set_is_primary(true).await?;
					}
				}
				Some(hit) if utils::ago_in_seconds(primary_promotion) > hit.updated_at => {
					// attempt to upgrade to primary (set is_primary on the next iteration)
					Config::set_where::<_, Uuid>(db, ConfigKey::Primary, uuid, hit).await?;
				}
				_ => {
					// either cool-down period has started or this is a secondary
					self.app.set_is_primary(false).await?;
				}
			}

			sleep(Duration::from_secs(self.app.settings.primary_ping)).await
		}
	}

	fn log(&self, index_type: IndexType, message: &str) {
		println!(
			"{} {}: {message}",
			style("Indexer").cyan().bold(),
			style(format!("({index_type})")).dim()
		);
	}

	fn format_number(&self, n: usize) -> Result<String> {
		let locale = SystemLocale::default()?;
		Ok(n.to_formatted_string(&locale))
	}
}

pub struct Pipe {
	config_key: ConfigKey,
	sender: Sender<(ConfigKey, JsonValue, WarehouseData)>,
	receipt: mpsc::Receiver<()>,
	pub abort: broadcast::Receiver<()>,
}

impl Pipe {
	pub fn new(
		config_key: ConfigKey,
		sender: Sender<(ConfigKey, JsonValue, WarehouseData)>,
		receipt: mpsc::Receiver<()>,
		abort: broadcast::Receiver<()>,
	) -> Self {
		Self { config_key, sender, receipt, abort }
	}

	pub async fn push(
		&mut self,
		config_value: JsonValue,
		warehouse_data: WarehouseData,
	) -> Result<()> {
		self.sender.send((self.config_key, config_value, warehouse_data)).await?;

		tokio::select! {
			_ = self.receipt.recv() => {}
			_ = self.abort.recv() => {}
		}

		Ok(())
	}
}
