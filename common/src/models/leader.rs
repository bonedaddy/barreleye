use eyre::Result;
use sea_orm::{entity::prelude::*, QueryOrder, Set};
use serde::{Deserialize, Serialize};

use crate::{
	models::{BasicModel, PrimaryId},
	utils,
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

	pub async fn get_last_leader(
		db: &DatabaseConnection,
	) -> Result<Option<Self>> {
		Ok(Entity::find()
			.filter(Column::UpdatedAt.gte(utils::ago_in_seconds(60)))
			.order_by_desc(Column::UpdatedAt)
			.one(db)
			.await?)
	}
}
