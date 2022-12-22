use async_trait::async_trait;
use ethers::{
	self,
	abi::AbiDecode,
	prelude::*,
	types::{Address, Log, Transaction, TransactionReceipt, U256, U64},
	utils::hex::ToHex,
};
use eyre::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::{
	cache::CacheKey,
	chain::{ChainTrait, ModuleTrait, WarehouseData},
	models::Network,
	utils, BlockHeight, Cache, ChainModuleId, RateLimiter,
};
use modules::{EvmBalance, EvmErc20Balance, EvmErc20Transfer, EvmModuleTrait, EvmTransfer};

mod modules;

#[derive(Debug, Eq, PartialEq)]
pub enum EvmTopic {
	Unknown,
	Erc20Transfer(Address, Address, U256),
}

pub struct Evm {
	_cache: Arc<RwLock<Cache>>,
	network: Network,
	rpc: Option<String>,
	provider: Option<Arc<Provider<RetryClient<Http>>>>,
	rate_limiter: Option<Arc<RateLimiter>>,
}

impl Evm {
	pub fn new(cache: Arc<RwLock<Cache>>, network: Network) -> Self {
		let rps = network.rps as u32;

		Self {
			_cache: cache,
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
		vec![
			ChainModuleId::EvmTransfer,
			ChainModuleId::EvmBalance,
			ChainModuleId::EvmErc20Transfer,
			ChainModuleId::EvmErc20Balance,
		]
	}

	fn get_rate_limiter(&self) -> Option<Arc<RateLimiter>> {
		self.rate_limiter.clone()
	}

	fn format_address(&self, address: &str) -> String {
		if address.len() > 2 {
			if let Ok(parsed_address) = address[2..].parse() {
				return ethers::utils::to_checksum(&parsed_address, None);
			}
		}

		address.to_string()
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
		let provider = self.provider.as_ref().unwrap();

		self.rate_limit().await;
		match provider.get_block_with_txs(block_height).await? {
			Some(block) if block.number.is_some() => {
				let mut warehouse_data = WarehouseData::new();

				for tx in block.transactions.into_iter() {
					// skip if pending
					if tx.block_hash.is_none() {
						continue;
					}

					// process tx only if receipt exists
					self.rate_limit().await;
					if let Some(receipt) = provider.get_transaction_receipt(tx.hash()).await? {
						// skip if tx reverted
						if let Some(status) = receipt.status {
							if status == U64::zero() {
								continue;
							}
						}

						// process tx
						warehouse_data += self
							.process_transaction(
								block_height,
								block.timestamp.as_u32(),
								tx,
								receipt,
								modules.clone(),
							)
							.await?;
					}
				}

				ret = Some(warehouse_data);
			}
			_ => {}
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
		receipt: TransactionReceipt,
		mods: Vec<ChainModuleId>,
	) -> Result<WarehouseData> {
		let mut ret = WarehouseData::new();

		let mut modules: Vec<Box<dyn EvmModuleTrait>> = vec![
			Box::new(EvmTransfer::new(self.network.network_id)),
			Box::new(EvmBalance::new(self.network.network_id)),
			Box::new(EvmErc20Transfer::new(self.network.network_id)),
			Box::new(EvmErc20Balance::new(self.network.network_id)),
		];

		modules.retain(|m| mods.contains(&m.get_id()));

		for module in modules.into_iter() {
			ret += module.run(self, block_height, block_time, tx.clone(), receipt.clone()).await?;
		}

		Ok(ret)
	}

	async fn _is_smart_contract(&self, address: &H160) -> Result<bool> {
		let cache_key = CacheKey::EvmSmartContract(
			self.network.network_id as u64,
			ethers::utils::to_checksum(address, None),
		);

		Ok(match self._cache.read().await.get::<bool>(cache_key.clone()).await? {
			Some(v) => v,
			_ => {
				self.rate_limit().await;
				let is_smart_contract =
					!self.provider.as_ref().unwrap().get_code(*address, None).await?.is_empty();
				self._cache.read().await.set::<bool>(cache_key, is_smart_contract).await?;
				is_smart_contract
			}
		})
	}

	fn get_topic(&self, log: &Log) -> Result<EvmTopic> {
		if log.topics.len() == 3 &&
			log.topics[0].encode_hex::<String>() ==
				*"ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"
		{
			let from = Address::from(log.topics[1]);
			let to = Address::from(log.topics[2]);
			let amount = U256::decode(log.data.clone()).unwrap_or_default();

			return Ok(EvmTopic::Erc20Transfer(from, to, amount));
		}

		Ok(EvmTopic::Unknown)
	}
}
