use async_trait::async_trait;
use eyre::Result;
use indicatif::ProgressBar;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

use crate::ChainTrait;
use barreleye_common::models::Network;

pub struct Bitcoin {
	network: Network,
}

impl Bitcoin {
	pub async fn new(network: Network, _pb: &ProgressBar) -> Result<Self> {
		Ok(Self { network })
	}
}

#[async_trait]
impl ChainTrait for Bitcoin {
	fn get_network(&self) -> Network {
		self.network.clone()
	}

	fn get_rpc(&self) -> Option<String> {
		None
	}

	async fn watch(&self, _db: Arc<DatabaseConnection>) -> Result<()> {
		loop {
			// println!("new block @ bitcoin, {}", self.network.id); // @TODO
			sleep(Duration::from_secs(self.network.expected_block_time as u64))
				.await;
		}
	}
}
