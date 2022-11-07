use eyre::Result;
use sea_orm::{
	entity::{prelude::*, *},
	query::JoinType,
	QuerySelect,
};
use serde::{Deserialize, Serialize};

use crate::{
	models::{api_key, BasicModel, PrimaryId},
	utils, IdPrefix,
};

#[derive(
	Clone, Debug, PartialEq, Eq, Serialize, Deserialize, DeriveEntityModel,
)]
#[sea_orm(table_name = "accounts")]
pub struct Model {
	#[sea_orm(primary_key)]
	#[serde(skip_deserializing)]
	pub account_id: PrimaryId,
	pub id: String,
	pub name: String,
	#[sea_orm(nullable)]
	pub updated_at: Option<DateTime>,
	pub created_at: DateTime,
}

pub use ActiveModel as AccountActiveModel;
pub use Model as Account;

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {
	ApiKey,
}

impl RelationTrait for Relation {
	fn def(&self) -> RelationDef {
		match self {
			Self::ApiKey => Entity::belongs_to(api_key::Entity)
				.from(Column::AccountId)
				.to(api_key::Column::AccountId)
				.into(),
		}
	}
}

impl ActiveModelBehavior for ActiveModel {}

impl BasicModel for Model {
	type ActiveModel = ActiveModel;
}

impl Model {
	pub fn new_model(name: &str) -> ActiveModel {
		ActiveModel {
			id: Set(utils::new_unique_id(IdPrefix::Account)),
			name: Set(name.to_string()),
			..Default::default()
		}
	}

	pub async fn get_by_api_key(
		db: &DatabaseConnection,
		api_key: Uuid,
	) -> Result<Option<Self>> {
		Ok(Entity::find()
			.join(JoinType::LeftJoin, Relation::ApiKey.def())
			.filter(api_key::Column::Uuid.eq(api_key))
			.one(db)
			.await?)
	}
}
