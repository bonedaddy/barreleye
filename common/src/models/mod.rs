use async_trait::async_trait;
use eyre::Result;
use sea_orm::{
	entity::prelude::*,
	query::*,
	sea_query::{types::*, Expr},
	FromQueryResult,
};

pub mod sanctioned_address;

pub use sanctioned_address::{SanctionedAddress, SanctionedAddressActiveModel};

#[async_trait]
pub trait BasicModel {
	type ActiveModel: ActiveModelTrait + ActiveModelBehavior + Sized + Send;

	async fn count_all(db: &DatabaseConnection) -> Result<i64> {
		#[derive(FromQueryResult)]
		struct Count {
			count: i64,
		}

		let res = <Self::ActiveModel as ActiveModelTrait>::Entity::find()
			.select_only()
			.column_as(Expr::col(Alias::new("id")).count(), "count")
			.into_model::<Count>()
			.all(db)
			.await?;

		if !res.is_empty() {
			return Ok(res[0].count);
		}

		Ok(0)
	}
}
