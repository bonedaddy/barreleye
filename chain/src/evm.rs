use async_trait::async_trait;
use ethers::{
	abi::AbiEncode, prelude::*, types::Transaction as EvmTransaction,
};
use eyre::{bail, Result};
use indicatif::ProgressBar;
use std::sync::Arc;

use crate::{ChainTrait, IndexTransactionV1};
use barreleye_common::{
	models::{Cache, CacheKey, Network, Transaction},
	utils, AppState,
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
		let mut rpc: Option<String> = None;
		let mut maybe_provider: Option<Provider<Http>> = None;

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
			if let Ok(provider) = Provider::<Http>::try_from(url.clone()) {
				if provider.get_block_number().await.is_ok() {
					rpc = Some(url);
					maybe_provider = Some(provider);
				}
			}
		}

		if maybe_provider.is_none() {
			if let Some(pb) = pb {
				pb.abandon();
			}

			bail!(format!("{}: {}", network.name, message_failed));
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
			match self.provider.get_block_with_txs(block_height).await? {
				Some(block) if block.number.is_some() => {
					for tx in block.transactions.into_iter() {
						if let Some(tx) =
							self.process_transaction_v1(tx).await?
						{
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
				_ => {}
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

impl Evm {
	// v1 tracks only eoa-to-eoa transfer of non-zero ether
	async fn process_transaction_v1(
		&self,
		tx: EvmTransaction,
	) -> Result<Option<IndexTransactionV1>> {
		// skip if pending
		if tx.block_hash.is_none() {
			return Ok(None);
		}

		// skip if no asset transfer
		if tx.value.is_zero() {
			return Ok(None);
		}

		// skip if contract deploy call
		if tx.to.is_none() {
			return Ok(None);
		}

		// skip if contract fn call
		let to = tx.to.unwrap();
		let block_id = BlockId::Hash(tx.block_hash.unwrap());
		if !self.provider.get_code(to, Some(block_id)).await?.is_empty() {
			return Ok(None);
		}

		// skip if contract is sending funds
		if !self.provider.get_code(tx.from, Some(block_id)).await?.is_empty() {
			return Ok(None);
		}

		Ok(Some(IndexTransactionV1 {
			hash: tx.hash.encode_hex(),
			from: ethers::utils::to_checksum(&tx.from, None),
			to: ethers::utils::to_checksum(&to, None),
			value: tx.value.to_string(),
		}))
	}
}
