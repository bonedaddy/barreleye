use eyre::Result;
// use sea_orm::{entity::prelude::*, QueryOrder, Set};
use sea_orm::{
	entity::prelude::*,
	query::*,
	sea_query::{types::*, Expr},
	Set,
};
use serde::{Deserialize, Serialize};

use crate::{
	models::{BasicModel, PrimaryId},
	utils, Db,
};

#[derive(
	Clone, Debug, PartialEq, Eq, Serialize, Deserialize, DeriveEntityModel,
)]
#[sea_orm(table_name = "leaders")]
#[serde(rename_all = "camelCase")]
pub struct Model {
	#[sea_orm(primary_key)]
	#[serde(skip_serializing, skip_deserializing)]
	pub leader_id: PrimaryId,
	pub uuid: Uuid,
	#[serde(skip_serializing)]
	pub updated_at: DateTime,
	pub created_at: DateTime,
}

pub use ActiveModel as LeaderActiveModel;
pub use Model as Leader;

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {}

impl RelationTrait for Relation {
	fn def(&self) -> RelationDef {
		panic!("No RelationDef")
	}
}

impl ActiveModelBehavior for ActiveModel {}

impl BasicModel for Model {
	type ActiveModel = ActiveModel;
}

impl Model {
	pub fn new_model(uuid: Uuid) -> ActiveModel {
		ActiveModel { uuid: Set(uuid), ..Default::default() }
	}

	pub async fn get_active(db: &Db, since: u64) -> Result<Option<Self>> {
		Ok(Entity::find()
			.filter(Column::UpdatedAt.gte(utils::ago_in_seconds(since)))
			.order_by_desc(Column::UpdatedAt)
			.one(db.get())
			.await?)
	}

	pub async fn get_last(db: &Db) -> Result<Option<Self>> {
		Ok(Entity::find()
			.order_by_desc(Column::UpdatedAt)
			.one(db.get())
			.await?)
	}

	pub async fn check_in(db: &Db, uuid: Uuid) -> Result<bool, DbErr> {
		let res = Entity::update_many()
			.col_expr(Alias::new("updated_at"), Expr::value(utils::now()))
			.filter(Expr::col(Alias::new("uuid")).eq(uuid))
			.exec(db.get())
			.await?;

		Ok(res.rows_affected == 1)
	}

	pub async fn truncate(db: &Db, since: u64) -> Result<()> {
		Entity::delete_many()
			.filter(
				Expr::col(Alias::new("updated_at"))
					.lt(utils::ago_in_seconds(since)),
			)
			.exec(db.get())
			.await?;

		Ok(())
	}
}
