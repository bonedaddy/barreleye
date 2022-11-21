use async_trait::async_trait;
use bitcoin::{
	blockdata::transaction::Transaction as BitcoinTransaction,
	hash_types::Txid, util::address::Address, Network as BitcoinNetwork,
};
use bitcoincore_rpc::{Auth, Client, RpcApi};
use eyre::{bail, Result};
use indicatif::ProgressBar;
use primitive_types::U256;
use std::{
	collections::HashMap,
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
};
use tokio::sync::mpsc::{Receiver, Sender};
use url::Url;

use crate::ChainTrait;
use barreleye_common::{
	cache::CacheKey,
	models::{Network, PrimaryId, Transfer},
	utils, AppState,
};

pub struct Bitcoin {
	app_state: Arc<AppState>,
	network: Network,
	rpc: Option<String>,
	client: Arc<Client>,
	bitcoin_network: BitcoinNetwork,
}

impl Bitcoin {
	pub async fn new(
		app_state: Arc<AppState>,
		network: Network,
		pb: Option<&ProgressBar>,
	) -> Result<Self> {
		let mut rpc: Option<String> = None;
		let mut maybe_client: Option<Client> = None;

		let mut rpc_endpoints = vec![];

		let (message_trying, message_failed) = if network.rpc.is_empty() {
			rpc_endpoints =
				serde_json::from_value(network.rpc_bootstraps.clone())?;
			(
				"trying rpc endpoints…".to_string(),
				"Could not connect to any RPC endpoint.".to_string(),
			)
		} else {
			rpc_endpoints.push(network.rpc.clone());
			(
				"connecting to rpc…".to_string(),
				format!(
					"Could not connect to RPC endpoint @ `{}`.",
					utils::with_masked_auth(&network.rpc)
				),
			)
		};

		if let Some(pb) = pb {
			pb.set_message(message_trying);
		}

		for url in rpc_endpoints.into_iter() {
			if let Ok(u) = Url::parse(&url) {
				let auth = match (u.username(), u.password()) {
					(username, Some(password)) => Auth::UserPass(
						username.to_string(),
						password.to_string(),
					),
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

			bail!(format!("{}: {}", network.name, message_failed));
		}

		let bitcoin_network =
			BitcoinNetwork::from_magic(network.chain_id as u32)
				.unwrap_or(BitcoinNetwork::Bitcoin);

		Ok(Self {
			app_state,
			network,
			rpc,
			client: Arc::new(maybe_client.unwrap()),
			bitcoin_network,
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

	async fn get_block_height(&self) -> Result<u64> {
		Ok(self.client.get_block_count()?)
	}

	async fn get_last_processed_block(&self) -> Result<u64> {
		Ok(Transfer::get_block_height(
			&self.app_state.warehouse,
			self.network.network_id,
		)
		.await?
		.unwrap_or(0))
	}

	async fn process_blocks(
		&self,
		last_read_block: u64,
		should_keep_going: Arc<AtomicBool>,
		i_am_done: Sender<PrimaryId>,
		mut receipt: Receiver<()>,
	) -> Result<(u64, Vec<Transfer>)> {
		let mut block_height = last_read_block;
		let mut transfers = vec![];

		let mut already_notified = false;

		while should_keep_going.load(Ordering::SeqCst) {
			block_height += 1;

			let block_hash = self.client.get_block_hash(block_height)?;
			let block = self.client.get_block(&block_hash)?;

			for tx in block.txdata.into_iter() {
				let mut new_transfers = self
					.process_transaction_v1(
						block_height,
						block_hash.to_string(),
						block.header.time,
						tx,
					)
					.await?;

				transfers.append(&mut new_transfers);
			}

			if !already_notified {
				i_am_done.send(self.network.network_id).await?;
				already_notified = receipt.recv().await.is_some();
			}
		}

		Ok((block_height, transfers))
	}
}

impl Bitcoin {
	// v1 tracks only address-to-address transfer of non-zero bitcoin
	async fn process_transaction_v1(
		&self,
		block_height: u64,
		block_hash: String,
		block_time: u32,
		tx: BitcoinTransaction,
	) -> Result<Vec<Transfer>> {
		let mut ret = vec![];

		// index outputs for quicker lookup later (even if coinbase tx)
		let all_outputs = self.index_transaction_outputs(&tx).await?;

		// skip if coinbase tx
		if tx.is_coin_base() {
			return Ok(ret);
		}

		let mut all_inputs = vec![];
		for txin in tx.input.iter() {
			let (txid, vout) =
				(txin.previous_output.txid, txin.previous_output.vout);

			if !txid.is_empty() {
				if let Some((a, v)) = self.get_utxo(txid, vout).await? {
					all_inputs.push((a, v))
				}
			}
		}

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

		let input_map = get_unique_addresses(all_inputs);
		let input_total: u64 = input_map.iter().map(|(_, v)| v).sum();

		let output_map = get_unique_addresses(all_outputs);
		let output_total: u64 = output_map.iter().map(|(_, v)| v).sum();

		for input in input_map.iter() {
			for output in output_map.iter() {
				let (from, to) = (input.0.clone(), output.0.clone());
				if from != to {
					let amount = ((*input.1 as f64 / input_total as f64) *
						*output.1 as f64)
						.round();

					ret.push(Transfer::new(
						self.network.network_id,
						block_height,
						block_hash.clone(),
						tx.txid().as_hash().to_string(),
						from.into(),
						to.into(),
						None,
						U256::from_str_radix(&amount.to_string(), 10)?,
						U256::from_str_radix(&output_total.to_string(), 10)?,
						block_time,
					));
				}
			}
		}

		Ok(ret)
	}

	async fn index_transaction_outputs(
		&self,
		tx: &BitcoinTransaction,
	) -> Result<Vec<(String, u64)>> {
		let mut ret = vec![];

		for (i, txout) in tx.output.iter().enumerate() {
			if let Some(a) = self.get_address(tx, i as u32)? {
				let cache_key = CacheKey::BitcoinTxIndex(
					self.network.network_id as u64,
					tx.txid().as_hash().to_string(),
					i as u32,
				);

				let v = txout.value;
				let cache_value = (a.to_string(), v);

				self.app_state
					.cache
					.set::<(String, u64)>(cache_key, cache_value)
					.await?;

				ret.push((a, v));
			}
		}

		Ok(ret)
	}

	async fn get_utxo(
		&self,
		txid: Txid,
		vout: u32,
	) -> Result<Option<(String, u64)>> {
		let cache_key = CacheKey::BitcoinTxIndex(
			self.network.network_id as u64,
			txid.as_hash().to_string(),
			vout,
		);

		let ret = match self
			.app_state
			.cache
			.get::<(String, u64)>(cache_key.clone())
			.await?
		{
			Some((a, v)) => {
				self.app_state.cache.delete(cache_key.clone()).await?;
				Some((a, v))
			}
			_ => {
				let tx = self.client.get_raw_transaction(&txid, None)?;
				self.get_address(&tx, vout)?.map(|a| {
					let v = tx.output[vout as usize].value;
					(a, v)
				})
			}
		};

		Ok(ret)
	}

	fn get_address(
		&self,
		tx: &BitcoinTransaction,
		vout: u32,
	) -> Result<Option<String>> {
		let mut ret = None;

		if vout < tx.output.len() as u32 {
			if let Ok(address) = Address::from_script(
				&tx.output[vout as usize].script_pubkey,
				self.bitcoin_network,
			) {
				ret = Some(address.to_string());
			} else {
				ret = Some(format!("{}:{}", tx.txid().as_hash(), vout));
			}
		}

		Ok(ret)
	}
}
