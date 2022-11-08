use async_trait::async_trait;
use sea_orm_migration::prelude::*;

use crate::LabelId;

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
					.col(
						ColumnDef::new(Labels::Id)
							.unique_key()
							.string()
							.not_null(),
					)
					.col(ColumnDef::new(Labels::Name).string().not_null())
					.col(ColumnDef::new(Labels::IsEnabled).boolean().not_null())
					.col(
						ColumnDef::new(Labels::IsHardcoded)
							.boolean()
							.not_null(),
					)
					.col(ColumnDef::new(Labels::IsTracked).boolean().not_null())
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
			.exec_stmt(
				Query::insert()
					.into_table(Labels::Table)
					.columns([
						Labels::Id,
						Labels::Name,
						Labels::IsEnabled,
						Labels::IsHardcoded,
						Labels::IsTracked,
					])
					.values_panic([
						LabelId::Ofac.to_string().into(),
						"OFAC".into(),
						true.into(),
						true.into(),
						true.into(),
					])
					.values_panic([
						LabelId::Ofsi.to_string().into(),
						"OFSI".into(),
						true.into(),
						true.into(),
						true.into(),
					])
					.on_conflict(
						OnConflict::columns([Labels::Id])
							.do_nothing()
							.to_owned(),
					)
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
	IsEnabled,
	IsHardcoded,
	IsTracked,
	UpdatedAt,
	CreatedAt,
}
