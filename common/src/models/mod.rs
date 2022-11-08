use async_trait::async_trait;
use eyre::Result;
use sea_orm::{
	entity::prelude::*,
	query::*,
	sea_query::{types::*, Expr, SimpleExpr},
	QuerySelect,
};

use crate::utils;

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

    async fn create(
		db: &DatabaseConnection,
		data: Self::ActiveModel,
	) -> Result<<<<Self::ActiveModel as ActiveModelTrait>::Entity as EntityTrait>::PrimaryKey as
	PrimaryKeyTrait>::ValueType>{
		let insert_result =
			<Self::ActiveModel as ActiveModelTrait>::Entity::insert(data)
				.exec(db)
				.await?;

		Ok(insert_result.last_insert_id)
	}

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

    async fn get_by_id(
		db: &DatabaseConnection,
		id: &str,
	) -> Result<Option<<<Self::ActiveModel as ActiveModelTrait>::Entity as EntityTrait>::Model>, DbErr>{
		<Self::ActiveModel as ActiveModelTrait>::Entity::find()
			.filter(Expr::col(Alias::new("id")).eq(id))
			.one(db)
			.await
	}

    async fn get_all(
		db: &DatabaseConnection,
	) -> Result<Vec<<<Self::ActiveModel as ActiveModelTrait>::Entity as EntityTrait>::Model>, DbErr>{
		Self::get_all_where(db, vec![], None, None).await
	}

    async fn get_all_where(
		db: &DatabaseConnection,
		conditions: Vec<SimpleExpr>,
		offset: Option<u64>,
		limit: Option<u64>,
	) -> Result<Vec<<<Self::ActiveModel as ActiveModelTrait>::Entity as EntityTrait>::Model>, DbErr>{
		let mut filter = Condition::all();
		for condition in conditions.into_iter() {
			filter = filter.add(condition);
		}

		let mut q = <Self::ActiveModel as ActiveModelTrait>::Entity::find()
			.filter(filter);

		if let Some(v) = offset {
			q = q.offset(v);
		}

		if let Some(v) = limit {
			q = q.limit(v);
		}

		q.all(db).await
	}

	async fn update_by_id(
		db: &DatabaseConnection,
		id: &str,
		data: Self::ActiveModel,
	) -> Result<bool, DbErr> {
		let res =
			<Self::ActiveModel as ActiveModelTrait>::Entity::update_many()
				.col_expr(Alias::new("updated_at"), Expr::value(utils::now()))
				.set(data)
				.filter(Expr::col(Alias::new("id")).eq(id))
				.exec(db)
				.await?;

		Ok(res.rows_affected == 1)
	}

	async fn delete(
		db: &DatabaseConnection,
		primary_id: <<<Self::ActiveModel as ActiveModelTrait>::Entity as EntityTrait>::PrimaryKey as
		PrimaryKeyTrait>::ValueType,
	) -> Result<bool, DbErr> {
		let res =
			<Self::ActiveModel as ActiveModelTrait>::Entity::delete_by_id(
				primary_id,
			)
			.exec(db)
			.await?;

		Ok(res.rows_affected == 1)
	}

	async fn delete_by_id(
		db: &DatabaseConnection,
		id: &str,
	) -> Result<bool, DbErr> {
		let res =
			<Self::ActiveModel as ActiveModelTrait>::Entity::delete_many()
				.filter(Expr::col(Alias::new("id")).eq(id))
				.exec(db)
				.await?;

		Ok(res.rows_affected == 1)
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
