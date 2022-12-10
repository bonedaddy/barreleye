use async_trait::async_trait;
use bitcoin::blockdata::transaction::Transaction;
use eyre::Result;
use primitive_types::U256;
use std::collections::HashMap;

use crate::{bitcoin::modules::BitcoinModuleTrait, Bitcoin, ModuleTrait, WarehouseData};
use barreleye_common::{
	models::{PrimaryId, Transfer},
	Address, BlockHeight, ChainModuleId,
};

pub struct BitcoinCoinbase {
	network_id: PrimaryId,
}

impl ModuleTrait for BitcoinCoinbase {
	fn new(network_id: PrimaryId) -> Self
	where
		Self: Sized,
	{
		Self { network_id }
	}

	fn get_id(&self) -> ChainModuleId {
		ChainModuleId::BitcoinCoinbase
	}
}

#[async_trait]
impl BitcoinModuleTrait for BitcoinCoinbase {
	async fn run(
		&self,
		_bitcoin: &Bitcoin,
		block_height: BlockHeight,
		block_time: u32,
		tx: Transaction,
		_inputs: HashMap<String, u64>,
		outputs: HashMap<String, u64>,
	) -> Result<WarehouseData> {
		let mut ret = WarehouseData::new();

		if tx.is_coin_base() {
			let tx_hash = tx.txid().as_hash().to_string();
			let output_amount_total: u64 = outputs.clone().into_values().sum();
			let batch_amount = U256::from_str_radix(&output_amount_total.to_string(), 10)?;

			for (to, amount) in outputs.into_iter() {
				ret.transfers.insert(Transfer::new(
					self.get_id(),
					self.network_id,
					block_height,
					tx_hash.clone(),
					Address::blank(),
					to.into(),
					None,
					U256::from_str_radix(&amount.to_string(), 10)?,
					batch_amount,
					block_time,
				));
			}
		}

		Ok(ret)
	}
}
