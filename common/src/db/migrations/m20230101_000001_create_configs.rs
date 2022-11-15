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
					.table(Config::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(Config::ConfigId)
							.big_integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(
						ColumnDef::new(Config::Key)
							.unique_key()
							.string()
							.not_null(),
					)
					.col(ColumnDef::new(Config::Value).string().null())
					.col(
						ColumnDef::new(Config::UpdatedAt)
							.date_time()
							.not_null()
							.extra("DEFAULT CURRENT_TIMESTAMP".to_owned()),
					)
					.col(
						ColumnDef::new(Config::CreatedAt)
							.date_time()
							.not_null()
							.extra("DEFAULT CURRENT_TIMESTAMP".to_owned()),
					)
					.to_owned(),
			)
			.await
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager.drop_table(Table::drop().table(Config::Table).to_owned()).await
	}
}

#[derive(Iden)]
enum Config {
	#[iden = "configs"]
	Table,
	ConfigId,
	Key,
	Value,
	UpdatedAt,
	CreatedAt,
}
