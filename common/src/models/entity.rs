use eyre::Result;
use sea_orm::{
	entity::prelude::*,
	sea_query::{func::Func, Expr},
	Condition, ConnectionTrait, Set,
};
use serde::{Deserialize, Serialize};

use crate::{
	models::{BasicModel, PrimaryId, SoftDeleteModel},
	utils, IdPrefix,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, DeriveEntityModel)]
#[sea_orm(table_name = "entities")]
#[serde(rename_all = "camelCase")]
pub struct Model {
	#[sea_orm(primary_key)]
	#[serde(skip_serializing, skip_deserializing)]
	pub entity_id: PrimaryId,
	pub id: String,
	#[sea_orm(nullable)]
	pub name: Option<String>,
	pub description: String,
	#[serde(skip_serializing)]
	pub is_deleted: bool,
	#[sea_orm(nullable)]
	#[serde(skip_serializing)]
	pub updated_at: Option<DateTime>,
	pub created_at: DateTime,

	#[sea_orm(ignore)]
	#[serde(skip_serializing_if = "Option::is_none")]
	pub tags: Option<Vec<String>>,
	#[sea_orm(ignore)]
	#[serde(skip_serializing_if = "Option::is_none")]
	pub addresses: Option<Vec<String>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SanitizedEntity {
	pub id: String,
	pub name: Option<String>,
	pub description: String,
}

impl From<Model> for SanitizedEntity {
	fn from(m: Model) -> SanitizedEntity {
		SanitizedEntity { id: m.id, name: m.name, description: m.description }
	}
}

pub use ActiveModel as LabeledEntityActiveModel;
pub use Model as LabeledEntity;

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

impl SoftDeleteModel for Model {
	type ActiveModel = ActiveModel;
}

impl Model {
	pub fn new_model(name: Option<String>, description: &str) -> ActiveModel {
		ActiveModel {
			id: Set(utils::new_unique_id(IdPrefix::Entity)),
			name: Set(name),
			description: Set(description.to_string()),
			is_deleted: Set(false),
			..Default::default()
		}
	}

	pub async fn get_by_name<C>(c: &C, name: &str, is_deleted: Option<bool>) -> Result<Option<Self>>
	where
		C: ConnectionTrait,
	{
		let mut q = Entity::find().filter(
			Condition::all()
				.add(Func::lower(Expr::col(Column::Name)).equals(name.trim().to_lowercase())),
		);

		if is_deleted.is_some() {
			q = q.filter(Column::IsDeleted.eq(is_deleted.unwrap()))
		}

		Ok(q.one(c).await?)
	}

	pub async fn get_all_by_entity_ids<C>(c: &C, entity_ids: Vec<PrimaryId>) -> Result<Vec<Self>>
	where
		C: ConnectionTrait,
	{
		Ok(Entity::find().filter(Column::EntityId.is_in(entity_ids)).all(c).await?)
	}

	pub async fn prune_all_by_entity_ids<C>(c: &C, entity_ids: Vec<PrimaryId>) -> Result<u64>
	where
		C: ConnectionTrait,
	{
		let res = Entity::delete_many()
			.filter(Column::IsDeleted.eq(true))
			.filter(Column::EntityId.is_in(entity_ids))
			.exec(c)
			.await?;
		Ok(res.rows_affected)
	}
}
