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
					.table(LabeledAddresses::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(LabeledAddresses::LabeledAddressId)
							.big_integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(
						ColumnDef::new(LabeledAddresses::LabelId)
							.big_integer()
							.not_null(),
					)
					.col(
						ColumnDef::new(LabeledAddresses::Id)
							.unique_key()
							.string()
							.not_null(),
					)
					.col(
						ColumnDef::new(LabeledAddresses::Address)
							.string()
							.not_null(),
					)
					.col(
						ColumnDef::new(LabeledAddresses::UpdatedAt)
							.date_time()
							.null(),
					)
					.col(
						ColumnDef::new(LabeledAddresses::CreatedAt)
							.date_time()
							.not_null()
							.extra("DEFAULT CURRENT_TIMESTAMP".to_owned()),
					)
					.foreign_key(
						&mut sea_query::ForeignKey::create()
							.name("fk_labeled_addresses_label_id")
							.from(
								LabeledAddresses::Table,
								LabeledAddresses::LabelId,
							)
							.to(Alias::new("labels"), Alias::new("label_id"))
							.to_owned(),
					)
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.if_not_exists()
					.name("ux_labeled_addresses_label_id_address")
					.table(LabeledAddresses::Table)
					.unique()
					.col(LabeledAddresses::LabelId)
					.col(LabeledAddresses::Address)
					.to_owned(),
			)
			.await
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.drop_table(Table::drop().table(LabeledAddresses::Table).to_owned())
			.await
	}
}

#[derive(Iden)]
enum LabeledAddresses {
	#[iden = "labeled_addresses"]
	Table,
	LabeledAddressId,
	LabelId,
	Id,
	Address,
	UpdatedAt,
	CreatedAt,
}
