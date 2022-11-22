use async_trait::async_trait;
use ethers::{
	abi::AbiEncode, prelude::*, types::Transaction as EvmTransaction,
	utils as ethers_utils,
};
use eyre::{bail, Result};
use indicatif::ProgressBar;
use primitive_types::U256;
use std::{
	borrow::BorrowMut,
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
};
use tokio::{
	sync::{mpsc::Sender, oneshot::Receiver},
	time::{sleep, Duration},
};

use crate::ChainTrait;
use barreleye_common::{
	cache::CacheKey,
	models::{Network, PrimaryId, Transfer},
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

	async fn get_block_height(&self) -> Result<u64> {
		Ok(self.provider.get_block_number().await?.as_u64())
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

			match self.provider.get_block_with_txs(block_height).await? {
				Some(block) if block.number.is_some() => {
					for tx in block.transactions.into_iter() {
						let mut new_transfers = self
							.process_transaction_v1(
								block_height,
								block.hash.unwrap().encode_hex(),
								block.timestamp.as_u32(),
								tx,
							)
							.await?;

						transfers.append(&mut new_transfers);
					}
				}
				_ => {
					break;
				}
			}

			if !already_notified {
				i_am_done.send(self.network.network_id).await?;
				already_notified = receipt.borrow_mut().await.is_ok();
			}
		}

		Ok((block_height, transfers))
	}
}

impl Evm {
	// v1 tracks only eoa-to-eoa transfer of non-zero ether
	async fn process_transaction_v1(
		&self,
		block_height: u64,
		block_hash: String,
		block_time: u32,
		tx: EvmTransaction,
	) -> Result<Vec<Transfer>> {
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
		let to = tx.to.unwrap();

		// skip if burning
		if to.is_zero() {
			return Ok(ret);
		}

		// skip if sending to self
		if tx.from == to {
			return Ok(ret);
		}

		// skip if contract fn call
		if self.is_smart_contract(&to).await? {
			return Ok(ret);
		}

		// skip if contract is sending funds
		if self.is_smart_contract(&tx.from).await? {
			return Ok(ret);
		}

		ret.push(Transfer::new(
			self.network.network_id,
			block_height,
			block_hash,
			tx.hash.encode_hex(),
			ethers_utils::to_checksum(&tx.from, None).into(),
			ethers_utils::to_checksum(&to, None).into(),
			None,
			U256::from_str_radix(&tx.value.to_string(), 10)?,
			U256::from_str_radix(&tx.value.to_string(), 10)?,
			block_time,
		));

		Ok(ret)
	}

	async fn is_smart_contract(&self, address: &H160) -> Result<bool> {
		let cache_key = CacheKey::EvmSmartContract(
			self.network.network_id as u64,
			ethers_utils::to_checksum(address, None),
		);

		Ok(match self.app_state.cache.get::<bool>(cache_key.clone()).await? {
			Some(v) => v,
			_ => {
				let is_smart_contract =
					!self.provider.get_code(*address, None).await?.is_empty();

				self.app_state
					.cache
					.set::<bool>(cache_key, is_smart_contract)
					.await?;

				is_smart_contract
			}
		})
	}
}
