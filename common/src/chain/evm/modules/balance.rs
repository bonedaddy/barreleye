use async_trait::async_trait;
use ethers::{
	abi::AbiEncode,
	types::{Transaction, TransactionReceipt},
	utils,
};
use eyre::Result;

use crate::{
	chain::{evm::modules::EvmModuleTrait, Evm, ModuleTrait, WarehouseData, U256},
	models::{Balance, PrimaryId},
	BlockHeight, ChainModuleId,
};

pub struct EvmBalance {
	network_id: PrimaryId,
}

impl ModuleTrait for EvmBalance {
	fn new(network_id: PrimaryId) -> Self
	where
		Self: Sized,
	{
		Self { network_id }
	}

	fn get_id(&self) -> ChainModuleId {
		ChainModuleId::EvmBalance
	}
}

#[async_trait]
impl EvmModuleTrait for EvmBalance {
	async fn run(
		&self,
		_evm: &Evm,
		block_height: BlockHeight,
		block_time: u32,
		tx: Transaction,
		_receipt: TransactionReceipt,
	) -> Result<WarehouseData> {
		let mut ret = WarehouseData::new();

		// skip if no asset transfer
		if tx.value.is_zero() {
			return Ok(ret);
		}

		// skip if contract deploy call
		if tx.to.is_none() {
			return Ok(ret);
		}

		// skip if sending to self
		if tx.from == tx.to.unwrap() {
			return Ok(ret);
		}

		ret.balances.insert(Balance::new(
			self.get_id(),
			self.network_id,
			block_height,
			tx.hash.encode_hex(),
			utils::to_checksum(&tx.from, None),
			None,
			U256::zero(),
			U256::from_str_radix(&tx.value.to_string(), 10)?,
			block_time,
		));
		ret.balances.insert(Balance::new(
			self.get_id(),
			self.network_id,
			block_height,
			tx.hash.encode_hex(),
			utils::to_checksum(&tx.to.unwrap(), None),
			None,
			U256::from_str_radix(&tx.value.to_string(), 10)?,
			U256::zero(),
			block_time,
		));

		Ok(ret)
	}
}
