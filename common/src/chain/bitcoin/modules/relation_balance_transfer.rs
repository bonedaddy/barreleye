use async_trait::async_trait;
use bitcoin::blockdata::transaction::Transaction;
use eyre::Result;
use std::collections::HashMap;

use crate::{
	chain::{bitcoin::modules::BitcoinModuleTrait, Bitcoin, ModuleId, ModuleTrait, WarehouseData},
	models::{PrimaryId, Relation, RelationReason},
	BlockHeight,
};

pub struct BitcoinRelationBalanceTransfer {
	network_id: PrimaryId,
}

impl ModuleTrait for BitcoinRelationBalanceTransfer {
	fn new(network_id: PrimaryId) -> Self {
		Self { network_id }
	}

	fn get_id(&self) -> ModuleId {
		ModuleId::BitcoinRelationBalanceTransfer
	}
}

#[async_trait]
impl BitcoinModuleTrait for BitcoinRelationBalanceTransfer {
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

		if outputs.len() == 1 {
			let tx_hash = tx.txid().as_hash().to_string();

			for input in inputs.iter() {
				for output in outputs.iter() {
					let (from, to) = (input.0.clone(), output.0.clone());

					if from != to &&
						bitcoin.is_valid_address(&from) &&
						bitcoin.is_valid_address(&to)
					{
						ret.relations.insert(Relation::new(
							self.get_id(),
							self.network_id,
							block_height,
							&tx_hash.clone(),
							&from,
							&to,
							RelationReason::WholeBalanceTransfer,
							block_time,
						));
					}
				}
			}
		}

		Ok(ret)
	}
}
