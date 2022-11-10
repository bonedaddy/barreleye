use eyre::Result;
use sea_orm::entity::{prelude::*, *};
use serde::{Deserialize, Serialize};

use crate::{
	models::{account, BasicModel, PrimaryId},
	utils, Db, IdPrefix,
};

#[derive(
	Clone, Debug, PartialEq, Eq, Serialize, Deserialize, DeriveEntityModel,
)]
#[sea_orm(table_name = "api_keys")]
#[serde(rename_all = "camelCase")]
pub struct Model {
	#[sea_orm(primary_key)]
	#[serde(skip_serializing, skip_deserializing)]
	pub api_key_id: PrimaryId,
	#[serde(skip_serializing)]
	pub account_id: PrimaryId,
	pub id: String,
	#[serde(skip_serializing)]
	pub uuid: Uuid,
	#[serde(skip_serializing)]
	pub is_admin: bool,
	#[sea_orm(nullable)]
	#[serde(skip_serializing)]
	pub updated_at: Option<DateTime>,
	pub created_at: DateTime,

	#[sea_orm(ignore)]
	pub key: String, // abbreviated `uuid` used in responses
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
	pub fn new_model(account_id: PrimaryId, is_admin: bool) -> ActiveModel {
		ActiveModel {
			account_id: Set(account_id),
			id: Set(utils::new_unique_id(IdPrefix::ApiKey)),
			uuid: Set(utils::new_uuid()),
			is_admin: Set(is_admin),
			..Default::default()
		}
	}

	pub async fn get_all_by_account_id(
		db: &Db,
		account_id: PrimaryId,
	) -> Result<Vec<Self>> {
		Ok(Entity::find()
			.filter(Column::AccountId.eq(account_id))
			.all(db.get())
			.await?)
	}

	pub fn format(&self) -> Self {
		Self { key: self.uuid.to_string()[..4].to_string(), ..self.clone() }
	}
}
