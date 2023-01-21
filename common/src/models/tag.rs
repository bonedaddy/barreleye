use eyre::Result;
use sea_orm::{
	entity::{prelude::*, *},
	ConnectionTrait,
};
use sea_orm_migration::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
	models::{BasicModel, PrimaryId},
	utils, IdPrefix,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, DeriveEntityModel)]
#[sea_orm(table_name = "tags")]
#[serde(rename_all = "camelCase")]
pub struct Model {
	#[sea_orm(primary_key)]
	#[serde(skip_serializing, skip_deserializing)]
	pub tag_id: PrimaryId,
	pub id: String,
	pub name: String,
	#[sea_orm(nullable)]
	#[serde(skip_serializing)]
	pub updated_at: Option<DateTime>,
	pub created_at: DateTime,
}

pub use ActiveModel as TagActiveModel;
pub use Model as Tag;

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
	pub fn new_model(name: &str) -> ActiveModel {
		ActiveModel {
			id: Set(utils::new_unique_id(IdPrefix::Tag)),
			name: Set(name.to_string()),
			..Default::default()
		}
	}

	pub async fn get_by_name<C>(c: &C, name: &str) -> Result<Option<Self>>
	where
		C: ConnectionTrait,
	{
		Ok(Entity::find()
			.filter(
				Condition::all()
					.add(Func::lower(Expr::col(Column::Name)).equals(name.trim().to_lowercase())),
			)
			.one(c)
			.await?)
	}
}
