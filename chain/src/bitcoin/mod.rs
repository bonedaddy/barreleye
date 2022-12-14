use async_trait::async_trait;
use bitcoin::{
	blockdata::transaction::Transaction, hash_types::Txid, util::address::Address,
	Network as BitcoinNetwork,
};
use eyre::{bail, Result};
use indicatif::ProgressBar;
use std::{collections::HashMap, sync::Arc};
use url::Url;

use crate::{ChainTrait, ModuleTrait, RateLimiter, WarehouseData};
use barreleye_common::{
	cache::CacheKey, models::Network, AppState, BlockHeight, ChainModuleId, Warehouse,
};
use client::{Auth, Client};
use modules::{BitcoinCoinbase, BitcoinLink, BitcoinModuleTrait, BitcoinTransfer, BitcoinTxAmount};

mod client;
mod modules;

pub struct Bitcoin {
	app_state: Arc<AppState>,
	network: Network,
	rpc: Option<String>,
	client: Arc<Client>,
	bitcoin_network: BitcoinNetwork,
	rate_limiter: Option<Arc<RateLimiter>>,
}

impl Bitcoin {
	pub async fn new(
		app_state: Arc<AppState>,
		network: Network,
		rate_limiter: Option<Arc<RateLimiter>>,
		pb: Option<&ProgressBar>,
	) -> Result<Self> {
		let mut rpc: Option<String> = None;
		let mut maybe_client: Option<Client> = None;

		let rpc_endpoints: Vec<String> = serde_json::from_value(network.rpc_endpoints.clone())?;

		if let Some(pb) = pb {
			pb.set_message("trying rpc endpoints…");
		}

		for url in rpc_endpoints.into_iter() {
			if let Ok(u) = Url::parse(&url) {
				let auth = match (u.username(), u.password()) {
					(username, Some(password)) => {
						Auth::UserPass(username.to_string(), password.to_string())
					}
					_ => Auth::None,
				};

				if let Ok(client) = Client::new(&url, auth) {
					if let Some(rate_limiter) = &rate_limiter {
						rate_limiter.until_ready().await;
					}

					if client.get_blockchain_info().await.is_ok() {
						rpc = Some(url);
						maybe_client = Some(client);
					}
				}
			}
		}

		if maybe_client.is_none() {
			if let Some(pb) = pb {
				pb.abandon();
			}

			bail!(format!("{}: Could not connect to any RPC endpoint.", network.name));
		}

		let bitcoin_network =
			BitcoinNetwork::from_magic(network.chain_id as u32).unwrap_or(BitcoinNetwork::Bitcoin);

		Ok(Self {
			app_state,
			network,
			rpc,
			client: Arc::new(maybe_client.unwrap()),
			bitcoin_network,
			rate_limiter,
		})
	}
}

#[async_trait]
impl ChainTrait for Bitcoin {
	fn get_warehouse(&self) -> Arc<Warehouse> {
		self.app_state.warehouse.clone()
	}

	fn get_network(&self) -> Network {
		self.network.clone()
	}

	fn get_rpc(&self) -> Option<String> {
		self.rpc.clone()
	}

	fn get_module_ids(&self) -> Vec<ChainModuleId> {
		vec![
			ChainModuleId::BitcoinTransfer,
			ChainModuleId::BitcoinTxAmount,
			ChainModuleId::BitcoinLink,
			ChainModuleId::BitcoinCoinbase,
		]
	}

	fn get_rate_limiter(&self) -> Option<Arc<RateLimiter>> {
		self.rate_limiter.clone()
	}

	async fn get_block_height(&self) -> Result<BlockHeight> {
		self.rate_limit().await;
		Ok(self.client.get_block_count().await?)
	}

	async fn process_block(
		&self,
		block_height: BlockHeight,
		modules: Vec<ChainModuleId>,
	) -> Result<Option<WarehouseData>> {
		let mut ret = None;

		self.rate_limit().await;
		if let Ok(block_hash) = self.client.get_block_hash(block_height).await {
			self.rate_limit().await;
			if let Ok(block) = self.client.get_block(&block_hash).await {
				let mut warehouse_data = WarehouseData::new();

				for tx in block.txdata.into_iter() {
					warehouse_data += self
						.process_transaction(block_height, block.header.time, tx, modules.clone())
						.await?;
				}

				ret = Some(warehouse_data);
			}
		}

		Ok(ret)
	}
}

impl Bitcoin {
	async fn process_transaction(
		&self,
		block_height: BlockHeight,
		block_time: u32,
		tx: Transaction,
		mods: Vec<ChainModuleId>,
	) -> Result<WarehouseData> {
		let mut ret = WarehouseData::new();

		let mut modules: Vec<Box<dyn BitcoinModuleTrait>> = vec![
			Box::new(BitcoinTransfer::new(self.network.network_id)),
			Box::new(BitcoinTxAmount::new(self.network.network_id)),
			Box::new(BitcoinLink::new(self.network.network_id)),
			Box::new(BitcoinCoinbase::new(self.network.network_id)),
		];

		modules.retain(|m| mods.contains(&m.get_id()));

		let get_unique_addresses = move |pair: Vec<(String, u64)>| {
			let mut m = HashMap::<String, u64>::new();

			for p in pair.into_iter() {
				let (address, value) = p;
				let address_key = address.to_string();

				let initial_value = m.get(&address_key).unwrap_or(&0);
				m.insert(address_key, *initial_value + value);
			}

			m
		};

		let inputs = get_unique_addresses({
			let mut ret = vec![];

			for txin in tx.input.iter() {
				let (txid, vout) = (txin.previous_output.txid, txin.previous_output.vout);

				if !txid.is_empty() && !tx.is_coin_base() {
					if let Some((a, v)) = self.get_utxo(txid, vout).await? {
						ret.push((a, v))
					}
				}
			}

			ret
		});

		let outputs =
			get_unique_addresses(self.index_transaction_outputs(block_height, &tx).await?);

		for module in modules.into_iter() {
			ret += module
				.run(self, block_height, block_time, tx.clone(), inputs.clone(), outputs.clone())
				.await?;
		}

		Ok(ret)
	}

	async fn index_transaction_outputs(
		&self,
		block_height: BlockHeight,
		tx: &Transaction,
	) -> Result<Vec<(String, u64)>> {
		let mut ret = vec![];

		for (i, txout) in tx.output.iter().enumerate() {
			if let Some(address) = self.get_address(tx, i as u32)? {
				let cache_key = CacheKey::BitcoinTxIndex(
					self.network.network_id as u64,
					tx.txid().as_hash().to_string(),
				);

				self.app_state.cache.read().await.set::<u64>(cache_key, block_height).await?;

				ret.push((address, txout.value));
			}
		}

		Ok(ret)
	}

	async fn get_utxo(&self, txid: Txid, vout: u32) -> Result<Option<(String, u64)>> {
		let cache_key =
			CacheKey::BitcoinTxIndex(self.network.network_id as u64, txid.as_hash().to_string());

		let mut block_hash = None;
		if let Some(block_height) =
			self.app_state.cache.read().await.get::<u64>(cache_key.clone()).await?
		{
			self.rate_limit().await;
			block_hash = Some(self.client.get_block_hash(block_height).await?);
			// @NOTE do not delete the "used up" utxo here; modules are stateless and another one
			// might need to use it
		}

		// `block_hash` will always be *some value* for those modules that have
		// started indexing from block 1; for all others -txindex is needed
		self.rate_limit().await;
		let tx = self.client.get_raw_transaction(&txid, block_hash.as_ref()).await?;
		let ret = self.get_address(&tx, vout)?.map(|a| {
			let v = tx.output[vout as usize].value;
			(a, v)
		});

		Ok(ret)
	}

	fn get_address(&self, tx: &Transaction, vout: u32) -> Result<Option<String>> {
		let mut ret = None;

		if vout < tx.output.len() as u32 {
			if let Ok(address) =
				Address::from_script(&tx.output[vout as usize].script_pubkey, self.bitcoin_network)
			{
				ret = Some(address.to_string());
			} else {
				ret = Some(format!("{}:{}", tx.txid().as_hash(), vout));
			}
		}

		Ok(ret)
	}

	fn is_valid_address(&self, address: &str) -> bool {
		!address.contains(':')
	}
}
