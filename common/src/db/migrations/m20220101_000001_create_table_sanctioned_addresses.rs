use async_trait::async_trait;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.create_table(
				Table::create()
					.table(SanctionedAddresses::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(
							SanctionedAddresses::SanctionedAddressId,
						)
						.big_integer()
						.not_null()
						.auto_increment()
						.primary_key(),
					)
					.col(
						ColumnDef::new(SanctionedAddresses::Id)
							.unique_key()
							.string()
							.not_null(),
					)
					.col(
						ColumnDef::new(SanctionedAddresses::Source)
							.string()
							.not_null(),
					)
					.col(
						ColumnDef::new(SanctionedAddresses::Address)
							.string()
							.not_null(),
					)
					.col(
						ColumnDef::new(SanctionedAddresses::Symbol)
							.string()
							.not_null(),
					)
					.col(
						ColumnDef::new(SanctionedAddresses::UpdatedAt)
							.date_time()
							.null(),
					)
					.col(
						ColumnDef::new(SanctionedAddresses::CreatedAt)
							.date_time()
							.not_null()
							.extra("DEFAULT CURRENT_TIMESTAMP".to_owned()),
					)
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("ix_sanctioned_addresses_address")
					.table(SanctionedAddresses::Table)
					.col(SanctionedAddresses::Address)
					.if_not_exists()
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("ux_sanctioned_addresses_source_address")
					.table(SanctionedAddresses::Table)
					.col(SanctionedAddresses::Source)
					.col(SanctionedAddresses::Address)
					.unique()
					.if_not_exists()
					.to_owned(),
			)
			.await
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.drop_table(
				Table::drop().table(SanctionedAddresses::Table).to_owned(),
			)
			.await
	}
}

#[derive(Iden)]
enum SanctionedAddresses {
	#[iden = "sanctioned_addresses"]
	Table,
	SanctionedAddressId,
	Id,
	Source,
	Address,
	Symbol,
	UpdatedAt,
	CreatedAt,
}
