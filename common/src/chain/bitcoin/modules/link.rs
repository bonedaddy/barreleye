use async_trait::async_trait;
use bitcoin::blockdata::transaction::Transaction;
use eyre::Result;
use std::collections::{HashMap, HashSet};

use crate::{
	chain::{bitcoin::modules::BitcoinModuleTrait, Bitcoin, ModuleId, ModuleTrait, WarehouseData},
	models::{Link, LinkReason, PrimaryId},
	BlockHeight,
};

pub struct BitcoinLink {
	network_id: PrimaryId,
}

impl ModuleTrait for BitcoinLink {
	fn new(network_id: PrimaryId) -> Self {
		Self { network_id }
	}

	fn get_id(&self) -> ModuleId {
		ModuleId::BitcoinLink
	}
}

#[async_trait]
impl BitcoinModuleTrait for BitcoinLink {
	async fn run(
		&self,
		bitcoin: &Bitcoin,
		block_height: BlockHeight,
		block_time: u32,
		tx: Transaction,
		inputs: HashMap<String, u64>,
		outputs: HashMap<String, u64>,
	) -> Result<WarehouseData> {
		let mut ret = WarehouseData::new();

		if tx.is_coin_base() {
			return Ok(ret);
		}

		let sends_change_to_self = {
			let inputs: HashSet<String> = inputs.clone().into_keys().collect();
			let outputs: HashSet<String> = outputs.clone().into_keys().collect();

			!inputs.intersection(&outputs).collect::<HashSet<_>>().is_empty()
		};

		if !sends_change_to_self {
			let tx_hash = tx.txid().as_hash().to_string();

			for input in inputs.iter() {
				for output in outputs.iter() {
					let (from, to) = (input.0.clone(), output.0.clone());

					if from != to &&
						bitcoin.is_valid_address(&from) &&
						bitcoin.is_valid_address(&to)
					{
						ret.links.insert(Link::new(
							self.get_id(),
							self.network_id,
							block_height,
							tx_hash.clone(),
							from,
							to,
							LinkReason::PossibleSelfTransfer,
							block_time,
						));
					}
				}
			}
		}

		Ok(ret)
	}
}
