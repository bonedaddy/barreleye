use sea_orm::entity::{prelude::*, *};
use serde::{Deserialize, Serialize};

use crate::{
	models::{account, BasicModel, PrimaryId},
	utils, IdPrefix,
};

#[derive(
	Clone, Debug, PartialEq, Eq, Serialize, Deserialize, DeriveEntityModel,
)]
#[sea_orm(table_name = "api_keys")]
pub struct Model {
	#[sea_orm(primary_key)]
	#[serde(skip_deserializing)]
	pub api_key_id: PrimaryId,
	pub account_id: PrimaryId,
	pub id: String,
	pub uuid: Uuid,
	#[sea_orm(nullable)]
	pub updated_at: Option<DateTime>,
	pub created_at: DateTime,
}

pub use ActiveModel as ApiKeyActiveModel;
pub use Model as ApiKey;

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {
	Account,
}

impl RelationTrait for Relation {
	fn def(&self) -> RelationDef {
		match self {
			Self::Account => Entity::belongs_to(account::Entity)
				.from(Column::AccountId)
				.to(account::Column::AccountId)
				.into(),
		}
	}
}

impl ActiveModelBehavior for ActiveModel {}

impl BasicModel for Model {
	type ActiveModel = ActiveModel;
}

impl Model {
	pub fn new_model(account_id: PrimaryId) -> ActiveModel {
		ActiveModel {
			account_id: Set(account_id),
			id: Set(utils::new_unique_id(IdPrefix::ApiKey)),
			uuid: Set(utils::new_uuid()),
			..Default::default()
		}
	}
}
