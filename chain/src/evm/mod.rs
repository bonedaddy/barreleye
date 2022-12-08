use async_trait::async_trait;
use ethers::{prelude::*, types::Transaction, utils};
use eyre::{bail, Result};
use indicatif::ProgressBar;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

use crate::{ChainTrait, ModuleTrait, RateLimiter, WarehouseData};
use barreleye_common::{
	cache::CacheKey,
	models::{Network, Transfer},
	AppState, BlockHeight, ChainModuleId,
};
use modules::{EvmModuleTrait, EvmTransfer};

mod modules;

pub struct Evm {
	app_state: Arc<AppState>,
	network: Network,
	rpc: Option<String>,
	provider: Arc<Provider<Http>>,
	rate_limiter: Option<Arc<RateLimiter>>,
}

impl Evm {
	pub async fn new(
		app_state: Arc<AppState>,
		network: Network,
		rate_limiter: Option<Arc<RateLimiter>>,
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

		Ok(Self {
			app_state,
			rate_limiter,
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

	fn get_module_ids(&self) -> Vec<ChainModuleId> {
		vec![ChainModuleId::EvmTransfer]
	}

	fn get_rate_limiter(&self) -> Option<Arc<RateLimiter>> {
		self.rate_limiter.clone()
	}

	async fn get_block_height(&self) -> Result<BlockHeight> {
		self.rate_limit().await;
		Ok(self.provider.get_block_number().await?.as_u64())
	}

	async fn get_last_processed_block(&self) -> Result<BlockHeight> {
		Ok(Transfer::get_block_height(&self.app_state.warehouse, self.network.network_id)
			.await?
			.unwrap_or(0))
	}

	async fn process_block(
		&self,
		block_height: BlockHeight,
		modules: Vec<ChainModuleId>,
	) -> Result<Option<WarehouseData>> {
		let mut ret = None;

		self.rate_limit().await;
		if let Some(block) = self.provider.get_block_with_txs(block_height).await? {
			if block.number.is_some() {
				let mut warehouse_data = WarehouseData::new();

				for tx in block.transactions.into_iter() {
					warehouse_data += self
						.process_transaction(
							block_height,
							block.timestamp.as_u32(),
							tx,
							modules.clone(),
						)
						.await?;
				}

				ret = Some(warehouse_data);
			}
		}

		Ok(ret)
	}
}

impl Evm {
	async fn process_transaction(
		&self,
		block_height: BlockHeight,
		block_time: u32,
		tx: Transaction,
		mods: Vec<ChainModuleId>,
	) -> Result<WarehouseData> {
		let mut ret = WarehouseData::new();

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
				self.rate_limit().await;
				let is_smart_contract = !self.provider.get_code(*address, None).await?.is_empty();
				self.app_state.cache.set::<bool>(cache_key, is_smart_contract).await?;
				is_smart_contract
			}
		})
	}
}
