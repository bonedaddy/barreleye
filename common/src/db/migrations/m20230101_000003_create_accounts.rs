use async_trait::async_trait;
use sea_orm_migration::prelude::*;

use crate::{utils, IdPrefix};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.create_table(
				Table::create()
					.table(Accounts::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(Accounts::AccountId)
							.big_integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(
						ColumnDef::new(Accounts::Id)
							.unique_key()
							.string()
							.not_null(),
					)
					.col(ColumnDef::new(Accounts::Name).string().not_null())
					.col(ColumnDef::new(Accounts::UpdatedAt).date_time().null())
					.col(
						ColumnDef::new(Accounts::CreatedAt)
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
					.into_table(Accounts::Table)
					.columns([Accounts::Id, Accounts::Name])
					.values_panic([
						utils::unique_id(IdPrefix::Account, "default").into(),
						"".into(),
					])
					.on_conflict(
						OnConflict::columns([Accounts::Id])
							.do_nothing()
							.to_owned(),
					)
					.to_owned(),
			)
			.await
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.drop_table(Table::drop().table(Accounts::Table).to_owned())
			.await
	}
}

#[derive(Iden)]
enum Accounts {
	#[iden = "accounts"]
	Table,
	AccountId,
	Id,
	Name,
	UpdatedAt,
	CreatedAt,
}
