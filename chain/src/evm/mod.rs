use async_trait::async_trait;
use ethers::{prelude::*, types::Transaction as EvmTransaction, utils};
use eyre::{bail, Result};
use indicatif::ProgressBar;
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

use crate::{ChainTrait, IndexResults, ModuleTrait};
use barreleye_common::{
	cache::CacheKey,
	models::{Network, PrimaryId, Transfer},
	AppState,
};
use modules::{EvmModuleTrait, EvmTransfer};

mod modules;

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

		let rpc_endpoints: Vec<String> =
			serde_json::from_value(network.rpc_endpoints.clone())?;

		if let Some(pb) = pb {
			pb.set_message("trying rpc endpoints…");
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

			bail!(format!(
				"{}: Could not connect to any RPC endpoint.",
				network.name
			));
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
	) -> Result<(u64, IndexResults)> {
		let mut block_height = last_read_block;
		let mut index_results = IndexResults::new();

		let mut already_notified = false;

		while should_keep_going.load(Ordering::SeqCst) {
			block_height += 1;

			match self.provider.get_block_with_txs(block_height).await? {
				Some(block) if block.number.is_some() => {
					for tx in block.transactions.into_iter() {
						index_results += self
							.process_transaction(
								block_height,
								block.timestamp.as_u32(),
								tx,
							)
							.await?;
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

		Ok((block_height, index_results))
	}
}

impl Evm {
	async fn process_transaction(
		&self,
		block_height: u64,
		block_time: u32,
		tx: EvmTransaction,
	) -> Result<IndexResults> {
		let mut ret = IndexResults::new();

		let modules: Vec<Box<dyn EvmModuleTrait>> =
			vec![Box::new(EvmTransfer::new(self.network.network_id))];

		for module in modules.into_iter() {
			ret +=
				module.run(self, block_height, block_time, tx.clone()).await?;
		}

		Ok(ret)
	}

	async fn is_smart_contract(&self, address: &H160) -> Result<bool> {
		let cache_key = CacheKey::EvmSmartContract(
			self.network.network_id as u64,
			utils::to_checksum(address, None),
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
