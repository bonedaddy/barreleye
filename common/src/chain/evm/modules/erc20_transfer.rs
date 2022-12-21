use async_trait::async_trait;
use ethers::{
	abi::{AbiDecode, AbiEncode},
	types::{Address, Transaction, TransactionReceipt, U64},
	utils,
};
use eyre::Result;

use crate::{
	chain::{
		evm::{modules::EvmModuleTrait, EvmTopic},
		Evm, ModuleTrait, WarehouseData, U256,
	},
	models::{PrimaryId, Transfer},
	BlockHeight, ChainModuleId,
};

pub struct EvmErc20Transfer {
	network_id: PrimaryId,
}

impl ModuleTrait for EvmErc20Transfer {
	fn new(network_id: PrimaryId) -> Self
	where
		Self: Sized,
	{
		Self { network_id }
	}

	fn get_id(&self) -> ChainModuleId {
		ChainModuleId::EvmErc20Transfer
	}
}

#[async_trait]
impl EvmModuleTrait for EvmErc20Transfer {
	async fn run(
		&self,
		evm: &Evm,
		block_height: BlockHeight,
		block_time: u32,
		tx: Transaction,
		receipt: TransactionReceipt,
	) -> Result<WarehouseData> {
		let mut ret = WarehouseData::new();

		// skip if tx reverted
		if let Some(status) = receipt.status {
			if status == U64::zero() {
				return Ok(ret);
			}
		}

		for log in receipt.logs.into_iter() {
			// if log was removed, it's not valid
			if let Some(removed) = log.removed {
				if removed {
					continue;
				}
			}

			// process erc20 `transfer` event
			if log.topics.len() == 3 && evm.get_topic(&log.topics[0]) == EvmTopic::Erc20Transfer {
				let from = Address::from(log.topics[1]);
				let to = Address::from(log.topics[2]);
				let amount = U256::decode(log.data).unwrap_or_default();

				if amount > U256::zero() {
					ret.transfers.insert(Transfer::new(
						self.get_id(),
						self.network_id,
						block_height,
						tx.hash.encode_hex(),
						utils::to_checksum(&from, None),
						utils::to_checksum(&to, None),
						Some(utils::to_checksum(&log.address, None)),
						amount,
						amount,
						block_time,
					));
				}
			}
		}

		Ok(ret)
	}
}
