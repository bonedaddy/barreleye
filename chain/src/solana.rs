use async_trait::async_trait;
use eyre::Result;
use indicatif::ProgressBar;
use std::sync::Arc;

use crate::ChainTrait;
use barreleye_common::{models::Network, AppState};

pub struct Solana {
	_app_state: Arc<AppState>,
	network: Network,
}

impl Solana {
	pub async fn new(
		app_state: Arc<AppState>,
		network: Network,
		_pb: &ProgressBar,
	) -> Result<Self> {
		Ok(Self { _app_state: app_state, network })
	}
}

#[async_trait]
impl ChainTrait for Solana {
	fn get_network(&self) -> Network {
		self.network.clone()
	}

	fn get_rpc(&self) -> Option<String> {
		None
	}

	async fn process_blocks(&self) -> Result<()> {
		// println!("processing blocks at {}…", self.network.id); // @TODO
		Ok(())
	}
}
