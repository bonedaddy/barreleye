use async_trait::async_trait;
use ethers::prelude::*;
use eyre::{bail, Result};
use indicatif::ProgressBar;
use std::sync::Arc;

use crate::ChainTrait;
use barreleye_common::{models::Network, AppState};

pub struct Evm {
	_app_state: Arc<AppState>,
	network: Network,
	rpc: Option<String>,
	_provider: Arc<Provider<Http>>,
}

impl Evm {
	pub async fn new(
		app_state: Arc<AppState>,
		network: Network,
		pb: Option<&ProgressBar>,
	) -> Result<Self> {
		let abort = |s: &str| {
			if let Some(pb) = pb {
				pb.abandon();
			}

			bail!(format!("{}: {}", network.name, s));
		};

		let mut rpc: Option<String> = None;
		let mut maybe_provider: Option<Provider<Http>> = None;

		if network.rpc.is_empty() {
			if let Some(pb) = pb {
				pb.set_message("trying rpc endpoints…");
			}

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
			if let Some(pb) = pb {
				pb.set_message("connecting to rpc…");
			}

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

		Ok(Self {
			_app_state: app_state,
			network,
			rpc,
			_provider: Arc::new(maybe_provider.unwrap()),
		})
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

	async fn process_blocks(&self) -> Result<()> {
		// println!("processing blocks at {}…", self.network.id); // @TODO
		Ok(())
	}
}
