use async_trait::async_trait;
use bitcoin::{
	blockdata::transaction::Transaction as BitcoinTransaction,
	util::address::Address,
};
use bitcoincore_rpc::{Auth, Client, RpcApi};
use eyre::{bail, Result};
use indicatif::ProgressBar;
use std::sync::Arc;
use url::Url;

use crate::{ChainTrait, IndexTransactionV1};
use barreleye_common::{
	models::{Cache, CacheKey, Network, Transaction},
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

		let last_indexed_block_height = {
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

		let from_block_height = last_indexed_block_height + 1;
		let to_block_height = last_indexed_block_height + 5;

		let mut txns = vec![];
		for block_height in from_block_height..=to_block_height {
			let block_hash = self.client.get_block_hash(block_height)?;
			let block = self.client.get_block(&block_hash)?;

			for tx in block.txdata.into_iter() {
				if let Some(tx) = self.process_transaction_v1(tx).await? {
					txns.push(Transaction::new(
						self.network.network_id,
						block_height,
						tx.hash,
						tx.from.into(),
						tx.to.into(),
						None,
						tx.value,
					));
				}
			}
		}

		if !txns.is_empty() {
			Transaction::create_many(&self.app_state.warehouse, txns).await?;
		}

		Cache::set::<u64>(&self.app_state.db, cache_key, to_block_height)
			.await?;

		Ok(())
	}
}

impl Bitcoin {
	// v1 tracks only address-to-address transfer of non-zero bitcoin
	async fn process_transaction_v1(
		&self,
		tx: BitcoinTransaction,
	) -> Result<Option<IndexTransactionV1>> {
		// skip if coinbase tx
		if tx.is_coin_base() {
			return Ok(None);
		}

		// get all inputs
		let input: Vec<(Address, u64)> = tx
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
							bitcoin::Network::Bitcoin,
						)
						.ok()
						.map(|a| (a, txout.value))
					} else {
						None
					}
				}
			})
			.collect();

		// get all outputs
		let output: Vec<(Address, u64)> = tx
			.output
			.iter()
			.filter_map(|txout| {
				Address::from_script(
					&txout.script_pubkey,
					bitcoin::Network::Bitcoin,
				)
				.ok()
				.map(|a| (a, txout.value))
			})
			.collect();

		// @TODO
		println!("in: {:?}, out: {:?}", input, output);

		Ok(None)
	}
}
