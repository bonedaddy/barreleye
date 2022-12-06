use async_trait::async_trait;
use ethers::{prelude::*, types::Transaction, utils};
use eyre::{bail, Result};
use indicatif::ProgressBar;
use std::sync::{
	atomic::{AtomicBool, Ordering},
	Arc,
};
use tokio::time::{sleep, Duration};

use crate::{CanExit, ChainTrait, IndexResults, ModuleTrait};
use barreleye_common::{
	cache::CacheKey,
	models::{Network, Transfer},
	AppState, ChainModuleId,
};
use modules::{EvmModuleTrait, EvmTransfer};

mod modules;

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
		let mut rpc: Option<String> = None;
		let mut maybe_provider: Option<Provider<Http>> = None;

		let rpc_endpoints: Vec<String> = serde_json::from_value(network.rpc_endpoints.clone())?;

		if let Some(pb) = pb {
			pb.set_message("trying rpc endpoints…");
		}

		for url in rpc_endpoints.into_iter() {
			if let Ok(provider) = Provider::<Http>::try_from(url.clone()) {
				let can_connect = tokio::select! {
					_ = sleep(Duration::from_secs(5)) => false,
					block = provider.get_block_number() => block.is_ok()
				};

				if can_connect {
					rpc = Some(url);
					maybe_provider = Some(provider);
				}
			}
		}

		if maybe_provider.is_none() {
			if let Some(pb) = pb {
				pb.abandon();
			}

			bail!(format!("{}: Could not connect to any RPC endpoint.", network.name));
		}

		Ok(Self { app_state, network, rpc, provider: Arc::new(maybe_provider.unwrap()) })
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

	fn get_module_ids(&self) -> Vec<ChainModuleId> {
		vec![ChainModuleId::EvmTransfer]
	}

	async fn get_block_height(&self) -> Result<u64> {
		Ok(self.provider.get_block_number().await?.as_u64())
	}

	async fn get_last_processed_block(&self) -> Result<u64> {
		Ok(Transfer::get_block_height(&self.app_state.warehouse, self.network.network_id)
			.await?
			.unwrap_or(0))
	}

	async fn process_blocks(
		&self,
		starting_block: u64,
		ending_block: Option<u64>,
		modules: Vec<ChainModuleId>,
		should_keep_going: Arc<AtomicBool>,
		mut can_exit: CanExit,
	) -> Result<(u64, IndexResults)> {
		let mut block_height = starting_block;
		let mut index_results = IndexResults::new();

		while should_keep_going.load(Ordering::SeqCst) {
			block_height += 1;

			if let Some(max_block_height) = ending_block {
				if block_height > max_block_height {
					break;
				}
			}

			match self.process_block(block_height, modules.clone()).await? {
				Some(data) => index_results += data,
				None => break,
			}

			can_exit.notify().await?;
		}

		Ok((block_height, index_results))
	}

	async fn process_block(
		&self,
		block_height: u64,
		modules: Vec<ChainModuleId>,
	) -> Result<Option<IndexResults>> {
		let mut ret = None;

		if let Some(block) = self.provider.get_block_with_txs(block_height).await? {
			if block.number.is_some() {
				let mut index_results = IndexResults::new();

				for tx in block.transactions.into_iter() {
					index_results += self
						.process_transaction(
							block_height,
							block.timestamp.as_u32(),
							tx,
							modules.clone(),
						)
						.await?;
				}

				ret = Some(index_results);
			}
		}

		Ok(ret)
	}
}

impl Evm {
	async fn process_transaction(
		&self,
		block_height: u64,
		block_time: u32,
		tx: Transaction,
		mods: Vec<ChainModuleId>,
	) -> Result<IndexResults> {
		let mut ret = IndexResults::new();

		let mut modules: Vec<Box<dyn EvmModuleTrait>> =
			vec![Box::new(EvmTransfer::new(self.network.network_id))];

		modules.retain(|m| mods.contains(&m.get_id()));

		for module in modules.into_iter() {
			ret += module.run(self, block_height, block_time, tx.clone()).await?;
		}

		Ok(ret)
	}

	async fn is_smart_contract(&self, address: &H160) -> Result<bool> {
		let cache_key = CacheKey::EvmSmartContract(
			self.network.network_id as u64,
			utils::to_checksum(address, None),
		);

		Ok(match self.app_state.cache.get::<bool>(cache_key.clone()).await? {
			Some(v) => v,
			_ => {
				let is_smart_contract = !self.provider.get_code(*address, None).await?.is_empty();

				self.app_state.cache.set::<bool>(cache_key, is_smart_contract).await?;

				is_smart_contract
			}
		})
	}
}
