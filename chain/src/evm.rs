use async_trait::async_trait;
use ethers::prelude::*;
use eyre::{bail, Result};
use indicatif::ProgressBar;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

use crate::ChainTrait;
use barreleye_common::{models::Network, Db};

pub struct Evm {
	network: Network,
	rpc: Option<String>,
	_provider: Arc<Provider<Http>>,
}

impl Evm {
	pub async fn new(network: Network, pb: &ProgressBar) -> Result<Self> {
		let abort = |s: &str| {
			pb.abandon();
			bail!(format!("{}: {}", network.name, s));
		};

		let mut rpc: Option<String> = None;
		let mut maybe_provider: Option<Provider<Http>> = None;

		if network.rpc.is_empty() {
			pb.set_message("trying rpc endpoints…");

			let rpc_endpoints: Vec<String> =
				serde_json::from_value(network.rpc_bootstraps.clone())?;

			for rpc_endpoint in rpc_endpoints.into_iter() {
				if let Ok(provider) =
					Provider::<Http>::try_from(rpc_endpoint.clone())
				{
					if provider.get_block_number().await.is_ok() {
						rpc = Some(rpc_endpoint);
						maybe_provider = Some(provider);
					}
				}
			}
		} else {
			pb.set_message("connecting to rpc…");

			let rpc_endpoint = network.rpc.clone();
			maybe_provider =
				match Provider::<Http>::try_from(rpc_endpoint.clone()) {
					Ok(provider)
						if provider.get_block_number().await.is_ok() =>
					{
						rpc = Some(rpc_endpoint);
						Some(provider)
					}
					_ => {
						return abort(&format!(
					"Could not connect to RPC endpoint at `{rpc_endpoint}`."
				))
					}
				};
		}

		if maybe_provider.is_none() {
			return abort("Could not connect to any RPC endpoint.");
		}

		let provider = Arc::new(maybe_provider.unwrap());

		Ok(Self { network, rpc, _provider: provider })
	}
}

#[async_trait]
impl ChainTrait for Evm {
	fn get_network(&self) -> Network {
		self.network.clone()
	}

	fn get_rpc(&self) -> Option<String> {
		self.rpc.clone()
	}

	async fn watch(&self, _db: Arc<Db>) -> Result<()> {
		loop {
			// println!("new block @ evm, {}", self.network.id); // @TODO
			sleep(Duration::from_secs(self.network.expected_block_time as u64))
				.await;
		}
	}
}
