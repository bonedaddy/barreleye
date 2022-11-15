use async_trait::async_trait;
use bitcoin::{
	blockdata::transaction::Transaction as BitcoinTransaction,
	util::address::Address, Network as BitcoinNetwork,
};
use bitcoincore_rpc::{Auth, Client, RpcApi};
use eyre::{bail, Result};
use indicatif::ProgressBar;
use primitive_types::U256;
use std::{collections::HashMap, sync::Arc};
use url::Url;

use crate::ChainTrait;
use barreleye_common::{
	models::{Cache, CacheKey, Network, Transfer},
	utils, AppState,
};

pub struct Bitcoin {
	app_state: Arc<AppState>,
	network: Network,
	rpc: Option<String>,
	client: Arc<Client>,
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

		Ok(Self {
			app_state,
			network,
			rpc,
			client: Arc::new(maybe_client.unwrap()),
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

	async fn process_blocks(&self) -> Result<()> {
		let cache_key =
			CacheKey::LastSavedBlock(self.network.network_id as u64)
				.to_string();

		let block_height = {
			match Cache::get::<u64>(&self.app_state.db, cache_key.clone())
				.await?
			{
				Some(hit) => hit.value,
				_ => Transfer::get_block_height(
					&self.app_state.warehouse,
					self.network.network_id,
				)
				.await?
				.unwrap_or(0),
			}
		} + 1;

		let mut transfers = vec![];

		let block_hash = self.client.get_block_hash(block_height)?;
		let block = self.client.get_block(&block_hash)?;

		for tx in block.txdata.into_iter() {
			for transfer in self
				.process_transaction_v1(
					block_height,
					block_hash.to_string(),
					tx,
				)
				.await?
			{
				transfers.push(transfer);
			}
		}

		if !transfers.is_empty() {
			Transfer::create_many(&self.app_state.warehouse, transfers).await?;
		}

		Cache::set::<u64>(&self.app_state.db, cache_key, block_height).await?;

		Ok(())
	}
}

impl Bitcoin {
	// v1 tracks only address-to-address transfer of non-zero bitcoin
	async fn process_transaction_v1(
		&self,
		block_height: u64,
		block_hash: String,
		tx: BitcoinTransaction,
	) -> Result<Vec<Transfer>> {
		let mut ret = vec![];

		let bitcoin_network =
			BitcoinNetwork::from_magic(self.network.chain_id as u32)
				.unwrap_or(BitcoinNetwork::Bitcoin);

		// skip if coinbase tx
		if tx.is_coin_base() {
			return Ok(ret);
		}

		let all_inputs: Vec<(Address, u64)> = tx
			.input
			.iter()
			.filter_map(|txin| match txin.previous_output.txid.is_empty() {
				true => None,
				_ => {
					let tx = self
						.client
						.get_raw_transaction(&txin.previous_output.txid, None)
						.unwrap();

					let vout = txin.previous_output.vout as usize;
					if vout < tx.output.len() {
						let txout = &tx.output[vout];

						Address::from_script(
							&txout.script_pubkey,
							bitcoin_network,
						)
						.ok()
						.map(|a| (a, txout.value))
					} else {
						None
					}
				}
			})
			.collect();

		let all_outputs: Vec<(Address, u64)> = tx
			.output
			.iter()
			.filter_map(|txout| {
				Address::from_script(&txout.script_pubkey, bitcoin_network)
					.ok()
					.map(|a| (a, txout.value))
			})
			.collect();

		let get_unique_addresses = move |pair: Vec<(Address, u64)>| {
			let mut m = HashMap::<String, u64>::new();

			for p in pair.into_iter() {
				let (address, value) = p;
				let address_key = address.to_string();

				let initial_value = if m.contains_key(&address_key) {
					m[&address_key]
				} else {
					0
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
				let amount = ((*input.1 as f64 / input_total as f64) *
					*output.1 as f64)
					.round();

				ret.push(Transfer::new(
					self.network.network_id,
					block_height,
					block_hash.clone(),
					tx.txid().as_hash().to_string(),
					input.0.clone().into(),
					output.0.clone().into(),
					None,
					U256::from_str_radix(&amount.to_string(), 10)?,
					U256::from_str_radix(&output_total.to_string(), 10)?,
				));
			}
		}

		Ok(ret)
	}
}
