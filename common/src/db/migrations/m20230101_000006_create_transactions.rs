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
					.table(Transactions::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(Transactions::TransactionId)
							.big_integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(
						ColumnDef::new(Transactions::Id)
							.unique_key()
							.string()
							.not_null(),
					)
					.col(
						ColumnDef::new(Transactions::Address)
							.string()
							.not_null(),
					)
					.col(ColumnDef::new(Transactions::Data).json().not_null())
					.col(
						ColumnDef::new(Transactions::UpdatedAt)
							.date_time()
							.null(),
					)
					.col(
						ColumnDef::new(Transactions::CreatedAt)
							.date_time()
							.not_null()
							.extra("DEFAULT CURRENT_TIMESTAMP".to_owned()),
					)
					.to_owned(),
			)
			.await
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.drop_table(Table::drop().table(Transactions::Table).to_owned())
			.await
	}
}

#[derive(Iden)]
enum Transactions {
	#[iden = "transactions"]
	Table,
	TransactionId,
	Id,
	Address,
	Data,
	UpdatedAt,
	CreatedAt,
}
