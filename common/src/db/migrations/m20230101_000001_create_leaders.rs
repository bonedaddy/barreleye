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
					.table(Leaders::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(Leaders::LeaderId)
							.big_integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(ColumnDef::new(Leaders::Uuid).uuid().not_null())
					.col(
						ColumnDef::new(Leaders::UpdatedAt)
							.date_time()
							.not_null()
							.extra("DEFAULT CURRENT_TIMESTAMP".to_owned()),
					)
					.col(
						ColumnDef::new(Leaders::CreatedAt)
							.date_time()
							.not_null()
							.extra("DEFAULT CURRENT_TIMESTAMP".to_owned()),
					)
					.to_owned(),
			)
			.await
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager.drop_table(Table::drop().table(Leaders::Table).to_owned()).await
	}
}

#[derive(Iden)]
enum Leaders {
	#[iden = "leaders"]
	Table,
	LeaderId,
	Uuid,
	UpdatedAt,
	CreatedAt,
}
