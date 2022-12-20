use async_trait::async_trait;
use ethers::{self, prelude::*, types::Transaction};
use eyre::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::{
	cache::CacheKey,
	chain::{ChainTrait, ModuleTrait, WarehouseData},
	models::Network,
	utils, BlockHeight, Cache, ChainModuleId, RateLimiter,
};
use modules::{EvmModuleTrait, EvmTransfer};

mod modules;

pub struct Evm {
	cache: Arc<RwLock<Cache>>,
	network: Network,
	rpc: Option<String>,
	provider: Option<Arc<Provider<RetryClient<Http>>>>,
	rate_limiter: Option<Arc<RateLimiter>>,
}

impl Evm {
	pub fn new(cache: Arc<RwLock<Cache>>, network: Network) -> Self {
		let rps = network.rps as u32;

		Self {
			cache,
			network,
			rpc: None,
			provider: None,
			rate_limiter: utils::get_rate_limiter(rps),
		}
	}
}

#[async_trait]
impl ChainTrait for Evm {
	async fn connect(&mut self) -> Result<bool> {
		let rpc_endpoints: Vec<String> =
			serde_json::from_value(self.network.rpc_endpoints.clone())?;

		for url in rpc_endpoints.into_iter() {
			if let Ok(provider) = Provider::<RetryClient<Http>>::new_client(&url, 10, 1_000) {
				if let Some(rate_limiter) = &self.rate_limiter {
					rate_limiter.until_ready().await;
				}

				if provider.get_block_number().await.is_ok() {
					self.rpc = Some(url);
					self.provider = Some(Arc::new(provider));

					break;
				}
			}
		}

		Ok(self.is_connected())
	}

	fn is_connected(&self) -> bool {
		self.provider.is_some()
	}

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

	fn format_address(&self, address: &str) -> String {
		match address[2..].parse() {
			Ok(parsed_address) => ethers::utils::to_checksum(&parsed_address, None),
			_ => address.to_string(),
		}
	}

	async fn get_block_height(&self) -> Result<BlockHeight> {
		self.rate_limit().await;
		Ok(self.provider.as_ref().unwrap().get_block_number().await?.as_u64())
	}

	async fn process_block(
		&self,
		block_height: BlockHeight,
		modules: Vec<ChainModuleId>,
	) -> Result<Option<WarehouseData>> {
		let mut ret = None;

		self.rate_limit().await;
		if let Some(block) =
			self.provider.as_ref().unwrap().get_block_with_txs(block_height).await?
		{
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
			ethers::utils::to_checksum(address, None),
		);

		Ok(match self.cache.read().await.get::<bool>(cache_key.clone()).await? {
			Some(v) => v,
			_ => {
				self.rate_limit().await;
				let is_smart_contract =
					!self.provider.as_ref().unwrap().get_code(*address, None).await?.is_empty();
				self.cache.read().await.set::<bool>(cache_key, is_smart_contract).await?;
				is_smart_contract
			}
		})
	}
}
