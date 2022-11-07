use async_trait::async_trait;
use eyre::Result;
use sea_orm::{
	entity::prelude::*,
	sea_query::{types::*, Expr},
};

// @TODO `https://github.com/SeaQL/sea-orm/issues/1068`
pub type PrimaryId = i64;

pub mod account;
pub use account::{Account, AccountActiveModel};

pub mod api_key;
pub use api_key::{ApiKey, ApiKeyActiveModel};

pub mod label;
pub use label::{Label, LabelActiveModel};

pub mod labeled_address;
pub use labeled_address::{LabeledAddress, LabeledAddressActiveModel};

pub mod network;
pub use network::{Network, NetworkActiveModel};

pub mod transaction;
pub use transaction::{Transaction, TransactionActiveModel};

#[async_trait]
pub trait BasicModel {
	type ActiveModel: ActiveModelTrait + ActiveModelBehavior + Sized + Send;

    async fn create_many(
		db: &DatabaseConnection,
		data: Vec<Self::ActiveModel>,
	) -> Result<<<<Self::ActiveModel as ActiveModelTrait>::Entity as EntityTrait>::PrimaryKey as
	PrimaryKeyTrait>::ValueType>{
		let insert_result =
			<Self::ActiveModel as ActiveModelTrait>::Entity::insert_many(data)
				.exec(db)
				.await?;

		Ok(insert_result.last_insert_id)
	}

    async fn get(
		db: &DatabaseConnection,
		primary_id: <<<Self::ActiveModel as ActiveModelTrait>::Entity as EntityTrait>::PrimaryKey as
		PrimaryKeyTrait>::ValueType,
	) -> Result<Option<<<Self::ActiveModel as ActiveModelTrait>::Entity as EntityTrait>::Model>, DbErr>{
		<Self::ActiveModel as ActiveModelTrait>::Entity::find_by_id(primary_id)
			.one(db)
			.await
	}

	async fn delete_by_ids(
		db: &DatabaseConnection,
		ids: Vec<String>,
	) -> Result<u64, DbErr> {
		let res =
			<Self::ActiveModel as ActiveModelTrait>::Entity::delete_many()
				.filter(Expr::col(Alias::new("id")).is_in(ids))
				.exec(db)
				.await?;

		Ok(res.rows_affected)
	}
}
