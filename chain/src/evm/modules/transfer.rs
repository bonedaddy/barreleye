use async_trait::async_trait;
use ethers::{abi::AbiEncode, types::Transaction, utils};
use eyre::Result;
use primitive_types::U256;

use crate::{evm::modules::EvmModuleTrait, Evm, ModuleTrait, WarehouseData};
use barreleye_common::{
	models::{PrimaryId, Transfer},
	BlockHeight, ChainModuleId,
};

pub struct EvmTransfer {
	network_id: PrimaryId,
}

impl ModuleTrait for EvmTransfer {
	fn new(network_id: PrimaryId) -> Self
	where
		Self: Sized,
	{
		Self { network_id }
	}

	fn get_id(&self) -> ChainModuleId {
		ChainModuleId::EvmTransfer
	}
}

#[async_trait]
impl EvmModuleTrait for EvmTransfer {
	async fn run(
		&self,
		evm: &Evm,
		block_height: BlockHeight,
		block_time: u32,
		tx: Transaction,
	) -> Result<WarehouseData> {
		let mut ret = WarehouseData::new();

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
		if evm.is_smart_contract(&to).await? {
			return Ok(ret);
		}

		// skip if contract is sending funds
		if evm.is_smart_contract(&tx.from).await? {
			return Ok(ret);
		}

		ret.transfers.insert(Transfer::new(
			self.get_id(),
			self.network_id,
			block_height,
			tx.hash.encode_hex(),
			utils::to_checksum(&tx.from, None).into(),
			utils::to_checksum(&to, None).into(),
			None,
			U256::from_str_radix(&tx.value.to_string(), 10)?,
			U256::from_str_radix(&tx.value.to_string(), 10)?,
			block_time,
		));

		Ok(ret)
	}
}
