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
					.table(Tags::Table)
					.if_not_exists()
					.col(ColumnDef::new(Tags::EntityId).big_integer().not_null())
					.col(ColumnDef::new(Tags::TagId).big_integer().not_null())
					.col(
						ColumnDef::new(Tags::CreatedAt)
							.date_time()
							.not_null()
							.extra("DEFAULT CURRENT_TIMESTAMP".to_owned()),
					)
					.primary_key(sea_query::Index::create().col(Tags::EntityId).col(Tags::TagId))
					.to_owned(),
			)
			.await
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager.drop_table(Table::drop().table(Tags::Table).to_owned()).await
	}
}

#[derive(Iden)]
enum Tags {
	#[iden = "entity_tag_map"]
	Table,
	EntityId,
	TagId,
	CreatedAt,
}
