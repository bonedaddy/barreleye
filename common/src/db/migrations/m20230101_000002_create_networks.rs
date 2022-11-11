use async_trait::async_trait;
use sea_orm_migration::prelude::*;
use serde_json::json;
use std::time::Duration;

use crate::{utils, Blockchain, Env, IdPrefix};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.create_table(
				Table::create()
					.table(Networks::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(Networks::NetworkId)
							.big_integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(
						ColumnDef::new(Networks::Id)
							.unique_key()
							.string()
							.not_null(),
					)
					.col(
						ColumnDef::new(Networks::Name)
							.unique_key()
							.string()
							.not_null(),
					)
					.col(ColumnDef::new(Networks::Tag).string().not_null())
					.col(
						ColumnDef::new(Networks::Env)
							.small_integer()
							.not_null(),
					)
					.col(
						ColumnDef::new(Networks::Blockchain)
							.small_integer()
							.not_null(),
					)
					.col(
						ColumnDef::new(Networks::ChainId)
							.big_integer()
							.not_null(),
					)
					.col(
						ColumnDef::new(Networks::BlockTimeMs)
							.big_integer()
							.not_null(),
					)
					.col(ColumnDef::new(Networks::Rpc).string().not_null())
					.col(
						ColumnDef::new(Networks::RpcBootstraps)
							.json()
							.not_null(),
					)
					.col(ColumnDef::new(Networks::UpdatedAt).date_time().null())
					.col(
						ColumnDef::new(Networks::CreatedAt)
							.date_time()
							.not_null()
							.extra("DEFAULT CURRENT_TIMESTAMP".to_owned()),
					)
					.to_owned(),
			)
			.await?;

		manager
			.exec_stmt(
				Query::insert()
					.into_table(Networks::Table)
					.columns([
						Networks::Id,
						Networks::Name,
						Networks::Tag,
						Networks::Env,
						Networks::Blockchain,
						Networks::ChainId,
						Networks::BlockTimeMs,
						Networks::Rpc,
						Networks::RpcBootstraps,
					])
					.values_panic([
						utils::unique_id(
							IdPrefix::Network,
							"bitcoin_localhost",
						)
						.into(),
						"Bitcoin Localhost".into(),
						"Bitcoin".into(),
						Env::Localhost.into(),
						Blockchain::Bitcoin.into(),
						1.into(),
						(Duration::from_secs(10 * 60).as_millis() as u64)
							.into(),
						"http://127.0.0.1:8333".into(),
						json!([]).into(),
					])
					.values_panic([
						utils::unique_id(IdPrefix::Network, "bitcoin_testnet")
							.into(),
						"Bitcoin Testnet".into(),
						"Bitcoin".into(),
						Env::Testnet.into(),
						Blockchain::Bitcoin.into(),
						1.into(),
						(Duration::from_secs(10 * 60).as_millis() as u64)
							.into(),
						"".into(),
						json!(["https://btc.getblock.io/testnet/",]).into(),
					])
					.values_panic([
						utils::unique_id(IdPrefix::Network, "bitcoin").into(),
						"Bitcoin".into(),
						"Bitcoin".into(),
						Env::Mainnet.into(),
						Blockchain::Bitcoin.into(),
						1.into(),
						(Duration::from_secs(10 * 60).as_millis() as u64)
							.into(),
						"".into(),
						json!(["https://btc.getblock.io/mainnet/",]).into(),
					])
					.values_panic([
						utils::unique_id(
							IdPrefix::Network,
							"ethereum_localhost",
						)
						.into(),
						"Ethereum Localhost".into(),
						"Ethereum".into(),
						Env::Localhost.into(),
						Blockchain::Evm.into(),
						1.into(),
						(Duration::from_secs(12).as_millis() as u64).into(),
						"http://127.0.0.1:8545".into(),
						json!([]).into(),
					])
					.values_panic([
						utils::unique_id(IdPrefix::Network, "ethereum_goerli")
							.into(),
						"Ethereum Goerli".into(),
						"Ethereum".into(),
						Env::Testnet.into(),
						Blockchain::Evm.into(),
						5.into(),
						(Duration::from_secs(12).as_millis() as u64).into(),
						"".into(),
						json!([
							"https://rpc.ankr.com/eth_goerli",
							"https://eth-goerli.public.blastapi.io",
						])
						.into(),
					])
					.values_panic([
						utils::unique_id(IdPrefix::Network, "ethereum").into(),
						"Ethereum".into(),
						"Ethereum".into(),
						Env::Mainnet.into(),
						Blockchain::Evm.into(),
						1.into(),
						(Duration::from_secs(12).as_millis() as u64).into(),
						"".into(),
						json!([
							"https://cloudflare-eth.com",
							"https://rpc.ankr.com/eth",
							"https://rpc.flashbots.net",
						])
						.into(),
					])
					.on_conflict(
						OnConflict::columns([Networks::Id])
							.do_nothing()
							.to_owned(),
					)
					.to_owned(),
			)
			.await
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.drop_table(Table::drop().table(Networks::Table).to_owned())
			.await
	}
}

#[derive(Iden)]
enum Networks {
	#[iden = "networks"]
	Table,
	NetworkId,
	Id,
	Name,
	Tag,
	Env,
	Blockchain,
	ChainId,
	BlockTimeMs,
	Rpc,
	RpcBootstraps,
	UpdatedAt,
	CreatedAt,
}
