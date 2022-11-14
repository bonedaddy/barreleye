use async_trait::async_trait;
use ethers::{
	abi::AbiEncode, prelude::*, types::Transaction as EvmTransaction,
};
use eyre::{bail, Result};
use indicatif::ProgressBar;
use std::sync::Arc;

use crate::{ChainTrait, IndexTransferV1};
use barreleye_common::{
	models::{Cache, CacheKey, Network, Transfer},
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

		let mut txns = vec![];

		match self.provider.get_block_with_txs(block_height).await? {
			Some(block) if block.number.is_some() => {
				for tx in block.transactions.into_iter() {
					for transfer in self.process_transaction_v1(tx).await? {
						txns.push(Transfer::new(
							self.network.network_id,
							block_height,
							block.hash.unwrap().encode_hex(),
							transfer.tx_hash,
							transfer.from_address.into(),
							transfer.to_address.into(),
							None,
							transfer.amount,
							transfer.batch_amount,
						));
					}
				}
			}
			_ => {}
		}

		if !txns.is_empty() {
			Transfer::create_many(&self.app_state.warehouse, txns).await?;
		}

		Cache::set::<u64>(&self.app_state.db, cache_key, block_height).await?;

		Ok(())
	}
}

impl Evm {
	// v1 tracks only eoa-to-eoa transfer of non-zero ether
	async fn process_transaction_v1(
		&self,
		tx: EvmTransaction,
	) -> Result<Vec<IndexTransferV1>> {
		let mut ret = vec![];

		// skip if pending
		if tx.block_hash.is_none() {
			return Ok(ret);
		}

		// skip if no asset transfer
		if tx.value.is_zero() {
			return Ok(ret);
		}

		// skip if contract deploy call
		if tx.to.is_none() {
			return Ok(ret);
		}

		// skip if contract fn call
		let to = tx.to.unwrap();
		let block_id = BlockId::Hash(tx.block_hash.unwrap());
		if !self.provider.get_code(to, Some(block_id)).await?.is_empty() {
			return Ok(ret);
		}

		// skip if contract is sending funds
		if !self.provider.get_code(tx.from, Some(block_id)).await?.is_empty() {
			return Ok(ret);
		}

		ret.push(IndexTransferV1 {
			tx_hash: tx.hash.encode_hex(),
			from_address: ethers::utils::to_checksum(&tx.from, None),
			to_address: ethers::utils::to_checksum(&to, None),
			amount: tx.value.to_string(),
			batch_amount: tx.value.to_string(),
		});

		Ok(ret)
	}
}
