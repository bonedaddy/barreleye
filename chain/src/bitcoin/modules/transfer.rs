use async_trait::async_trait;
use bitcoin::blockdata::transaction::Transaction;
use eyre::Result;
use primitive_types::U256;
use std::collections::HashMap;

use crate::{bitcoin::modules::BitcoinModuleTrait, Bitcoin, IndexResults, ModuleTrait};
use barreleye_common::{
	models::{PrimaryId, Transfer},
	ChainModuleId,
};

pub struct BitcoinTransfer {
	network_id: PrimaryId,
}

impl ModuleTrait for BitcoinTransfer {
	fn new(network_id: PrimaryId) -> Self
	where
		Self: Sized,
	{
		Self { network_id }
	}

	fn get_id(&self) -> ChainModuleId {
		ChainModuleId::BitcoinTransfer
	}
}

#[async_trait]
impl BitcoinModuleTrait for BitcoinTransfer {
	async fn run(
		&self,
		_bitcoin: &Bitcoin,
		block_height: u64,
		block_time: u32,
		tx: Transaction,
		inputs: HashMap<String, u64>,
		outputs: HashMap<String, u64>,
	) -> Result<IndexResults> {
		let mut ret = IndexResults::new();

		if tx.is_coin_base() {
			return Ok(ret);
		}

		let tx_hash = tx.txid().as_hash().to_string();
		let input_amount_total: u64 = inputs.clone().into_values().sum();
		let output_amount_total: u64 = outputs.clone().into_values().sum();
		let batch_amount = U256::from_str_radix(&output_amount_total.to_string(), 10)?;

		for input in inputs.iter() {
			for output in outputs.iter() {
				let (from, to) = (input.0.clone(), output.0.clone());
				if from != to {
					let amount =
						((*input.1 as f64 / input_amount_total as f64) * *output.1 as f64).round();

					ret.transfers.insert(Transfer::new(
						ChainModuleId::BitcoinTransfer,
						self.network_id,
						block_height,
						tx_hash.clone(),
						from.into(),
						to.into(),
						None,
						U256::from_str_radix(&amount.to_string(), 10)?,
						batch_amount,
						block_time,
					));
				}
			}
		}

		Ok(ret)
	}
}
