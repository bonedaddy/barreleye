use eyre::Result;
use serde_json::Value as JsonValue;
use tokio::sync::{
	broadcast,
	mpsc::{self, Sender},
};

use barreleye_common::{chain::WarehouseData, models::ConfigKey};
pub use indexer::Indexer;
pub use lists::Lists;

mod indexer;
mod lists;

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
