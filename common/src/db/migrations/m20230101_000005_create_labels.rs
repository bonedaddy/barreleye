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
					.table(Labels::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(Labels::LabelId)
							.big_integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(ColumnDef::new(Labels::Id).unique_key().string().not_null())
					.col(ColumnDef::new(Labels::Name).unique_key().string().not_null())
					.col(ColumnDef::new(Labels::Description).string().not_null())
					.col(ColumnDef::new(Labels::IsDeleted).boolean().not_null())
					.col(ColumnDef::new(Labels::UpdatedAt).date_time().null())
					.col(
						ColumnDef::new(Labels::CreatedAt)
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
					.if_not_exists()
					.name("ix_labels_is_deleted")
					.table(Labels::Table)
					.col(Labels::IsDeleted)
					.to_owned(),
			)
			.await
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager.drop_table(Table::drop().table(Labels::Table).to_owned()).await
	}
}

#[derive(Iden)]
enum Labels {
	#[iden = "labels"]
	Table,
	LabelId,
	Id,
	Name,
	Description,
	IsDeleted,
	UpdatedAt,
	CreatedAt,
}
