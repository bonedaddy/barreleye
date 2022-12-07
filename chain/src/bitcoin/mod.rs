use async_trait::async_trait;
use bitcoin::{
	blockdata::transaction::Transaction, hash_types::Txid, util::address::Address,
	Network as BitcoinNetwork,
};
use bitcoincore_rpc::{Auth, Client, RpcApi};
use eyre::{bail, Result};
use indicatif::ProgressBar;
use std::{
	collections::HashMap,
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
};
use url::Url;

use crate::{CanExit, ChainTrait, ModuleTrait, RateLimiter, WarehouseData};
use barreleye_common::{
	cache::CacheKey,
	models::{Network, Transfer},
	AppState, BlockHeight, ChainModuleId,
};
use modules::{BitcoinCoinbase, BitcoinLink, BitcoinModuleTrait, BitcoinTransfer};

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
					if client.get_blockchain_info().is_ok() {
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
	fn get_network(&self) -> Network {
		self.network.clone()
	}

	fn get_rpc(&self) -> Option<String> {
		self.rpc.clone()
	}

	fn get_module_ids(&self) -> Vec<ChainModuleId> {
		vec![
			ChainModuleId::BitcoinTransfer,
			ChainModuleId::BitcoinLink,
			ChainModuleId::BitcoinCoinbase,
		]
	}

	async fn get_block_height(&self) -> Result<BlockHeight> {
		if let Some(rate_limiter) = &self.rate_limiter {
			rate_limiter.until_ready().await;
		}

		Ok(self.client.get_block_count()?)
	}

	async fn get_last_processed_block(&self) -> Result<BlockHeight> {
		Ok(Transfer::get_block_height(&self.app_state.warehouse, self.network.network_id)
			.await?
			.unwrap_or(0))
	}

	async fn process_blocks(
		&self,
		starting_block: BlockHeight,
		ending_block: Option<BlockHeight>,
		modules: Vec<ChainModuleId>,
		should_keep_going: Arc<AtomicBool>,
		mut can_exit: CanExit,
	) -> Result<(BlockHeight, WarehouseData)> {
		let mut block_height = starting_block;
		let mut warehouse_data = WarehouseData::new();

		while should_keep_going.load(Ordering::SeqCst) {
			block_height += 1;

			if let Some(max_block_height) = ending_block {
				if block_height > max_block_height {
					break;
				}
			}

			match self.process_block(block_height, modules.clone()).await? {
				Some(data) => warehouse_data += data,
				None => break,
			}

			can_exit.notify().await?;
		}

		Ok((block_height, warehouse_data))
	}

	async fn process_block(
		&self,
		block_height: BlockHeight,
		modules: Vec<ChainModuleId>,
	) -> Result<Option<WarehouseData>> {
		let mut ret = None;

		if let Some(rate_limiter) = &self.rate_limiter {
			rate_limiter.until_ready().await;
		}

		if let Ok(block_hash) = self.client.get_block_hash(block_height) {
			if let Some(rate_limiter) = &self.rate_limiter {
				rate_limiter.until_ready().await;
			}

			if let Ok(block) = self.client.get_block(&block_hash) {
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
			Box::new(BitcoinLink::new(self.network.network_id)),
			Box::new(BitcoinCoinbase::new(self.network.network_id)),
		];

		modules.retain(|m| mods.contains(&m.get_id()));

		let get_unique_addresses = move |pair: Vec<(String, u64)>| {
			let mut m = HashMap::<String, u64>::new();

			for p in pair.into_iter() {
				let (address, value) = p;
				let address_key = address.to_string();

				let initial_value = match m.contains_key(&address_key) {
					true => m[&address_key],
					_ => 0,
				};

				m.insert(address_key, initial_value + value);
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

				self.app_state.cache.set::<u64>(cache_key, block_height).await?;

				ret.push((address, txout.value));
			}
		}

		Ok(ret)
	}

	async fn get_utxo(&self, txid: Txid, vout: u32) -> Result<Option<(String, u64)>> {
		let cache_key =
			CacheKey::BitcoinTxIndex(self.network.network_id as u64, txid.as_hash().to_string());

		let mut block_hash = None;
		if let Some(block_height) = self.app_state.cache.get::<u64>(cache_key.clone()).await? {
			if let Some(rate_limiter) = &self.rate_limiter {
				rate_limiter.until_ready().await;
			}

			block_hash = Some(self.client.get_block_hash(block_height)?);
			// @NOTE do not delete the "used up" utxo here; modules are stateless and another one
			// might need to use it
		}

		// `block_hash` will always be *some value* for those modules that have
		// started indexing from block 1; for all others -txindex is needed
		if let Some(rate_limiter) = &self.rate_limiter {
			rate_limiter.until_ready().await;
		}
		let tx = self.client.get_raw_transaction(&txid, block_hash.as_ref())?;
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
