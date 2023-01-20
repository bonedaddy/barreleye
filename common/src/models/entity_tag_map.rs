use eyre::Result;
use sea_orm::entity::{prelude::*, *};
use sea_orm_migration::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
	models::{entity, tag, BasicModel, PrimaryId},
	Db,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, DeriveEntityModel)]
#[sea_orm(table_name = "entity_tag_map")]
#[serde(rename_all = "camelCase")]
pub struct Model {
	#[sea_orm(primary_key)]
	#[serde(skip_serializing, skip_deserializing)]
	pub entity_id: PrimaryId,
	#[sea_orm(primary_key)]
	#[serde(skip_serializing, skip_deserializing)]
	pub tag_id: PrimaryId,
	pub created_at: DateTime,
}

pub use ActiveModel as EntityTagMapActiveModel;
pub use Model as EntityTagMap;

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {
	Entity,
	Tag,
}

impl RelationTrait for Relation {
	fn def(&self) -> RelationDef {
		match self {
			Self::Entity => Entity::belongs_to(entity::Entity)
				.from(Column::EntityId)
				.to(entity::Column::EntityId)
				.into(),
			Self::Tag => {
				Entity::belongs_to(tag::Entity).from(Column::TagId).to(tag::Column::TagId).into()
			}
		}
	}
}

impl ActiveModelBehavior for ActiveModel {}

impl BasicModel for Model {
	type ActiveModel = ActiveModel;
}

impl Model {
	pub fn new_model(entity_id: PrimaryId, tag_id: PrimaryId) -> ActiveModel {
		ActiveModel { entity_id: Set(entity_id), tag_id: Set(tag_id), ..Default::default() }
	}

	pub async fn create_many(db: &Db, data: Vec<ActiveModel>) -> Result<(PrimaryId, PrimaryId)> {
		let insert_result = Entity::insert_many(data)
			.on_conflict(
				OnConflict::columns([Column::EntityId, Column::TagId]).do_nothing().to_owned(),
			)
			.exec(db.get())
			.await?;

		Ok(insert_result.last_insert_id)
	}

	pub async fn delete_not_included_tags(
		db: &Db,
		entity_id: PrimaryId,
		tag_ids: Vec<PrimaryId>,
	) -> Result<u64> {
		let res = Entity::delete_many()
			.filter(Column::EntityId.eq(entity_id))
			.filter(Column::TagId.is_not_in(tag_ids))
			.exec(db.get())
			.await?;

		Ok(res.rows_affected)
	}

	pub async fn delete_all_by_entity_ids(db: &Db, entity_ids: Vec<PrimaryId>) -> Result<u64> {
		let res =
			Entity::delete_many().filter(Column::EntityId.is_in(entity_ids)).exec(db.get()).await?;

		Ok(res.rows_affected)
	}

	pub async fn delete_all_by_tag_ids(db: &Db, tag_ids: Vec<PrimaryId>) -> Result<u64> {
		let res =
			Entity::delete_many().filter(Column::TagId.is_in(tag_ids)).exec(db.get()).await?;

		Ok(res.rows_affected)
	}
}
