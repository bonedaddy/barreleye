use async_trait::async_trait;
use ethers::{abi::AbiEncode, prelude::*};
use eyre::{bail, Result};
use indicatif::ProgressBar;
use std::sync::Arc;

use crate::ChainTrait;
use barreleye_common::{
	models::{Cache, CacheKey, Network, Transaction},
	AppState,
};

pub struct Evm {
	app_state: Arc<AppState>,
	network: Network,
	rpc: Option<String>,
	provider: Arc<Provider<Http>>,
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
			app_state,
			network,
			rpc,
			provider: Arc::new(maybe_provider.unwrap()),
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
		let cache_key =
			CacheKey::LastSavedBlock(self.network.network_id as u64)
				.to_string();

		let block_height = {
			match Cache::get::<u64>(&self.app_state.db, cache_key.clone())
				.await?
			{
				Some(hit) => hit.value,
				_ => Transaction::get_latest_inserted_block(
					&self.app_state.warehouse,
					self.network.network_id,
				)
				.await?
				.unwrap_or(0),
			}
		};

		let mut txns = vec![];
		let up_to_block_height = block_height + 5;

		for i in (block_height + 1)..=up_to_block_height {
			if let Some(block) = self.provider.get_block_with_txs(i).await? {
				if block.number.is_some() {
					for tx in block.transactions.iter() {
						// skip if contract creation (for now)
						if tx.to.is_none() {
							continue;
						}

						// skip if contract call (for now)
						if !self
							.provider
							.get_code(tx.to.unwrap(), None)
							.await?
							.is_empty()
						{
							continue;
						}

						// skip if no asset transfer (for now)
						if tx.value.is_zero() {
							continue;
						}

						// add tx
						txns.push(Transaction::new(
							self.network.network_id,
							i,
							tx.hash.encode_hex(),
							tx.from.into(),
							tx.to.unwrap().into(),
							None,
							tx.value.to_string(),
						));
					}
				}
			}
		}

		if !txns.is_empty() {
			Transaction::create_many(&self.app_state.warehouse, txns).await?;
		}

		Cache::set::<u64>(&self.app_state.db, cache_key, up_to_block_height)
			.await?;

		Ok(())
	}
}
