use eyre::Result;
use sea_orm::entity::{prelude::*, *};
use serde::{Deserialize, Serialize};

use crate::{
	models::{BasicModel, PrimaryId},
	utils, IdPrefix,
};

#[derive(
	Clone, Debug, PartialEq, Eq, Serialize, Deserialize, DeriveEntityModel,
)]
#[sea_orm(table_name = "labels")]
pub struct Model {
	#[sea_orm(primary_key)]
	#[serde(skip_serializing)]
	#[serde(skip_deserializing)]
	pub label_id: PrimaryId,
	pub id: String,
	pub name: String,
	#[serde(skip_serializing)]
	pub is_enabled: bool,
	#[serde(skip_serializing)]
	pub is_hardcoded: bool,
	#[sea_orm(nullable)]
	#[serde(skip_serializing)]
	pub updated_at: Option<DateTime>,
	pub created_at: DateTime,
}

pub use ActiveModel as LabelActiveModel;
pub use Model as Label;

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
	pub fn new_model(name: String) -> ActiveModel {
		ActiveModel {
			id: Set(utils::new_unique_id(IdPrefix::Label)),
			name: Set(name),
			..Default::default()
		}
	}

	pub async fn get_all_enabled_and_hardcoded(
		db: &DatabaseConnection,
	) -> Result<Vec<Self>> {
		Ok(Entity::find()
			.filter(Column::IsEnabled.eq(true))
			.filter(Column::IsHardcoded.eq(true))
			.all(db)
			.await?)
	}
}
