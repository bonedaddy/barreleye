use sea_orm::entity::{prelude::*, *};
use serde::{Deserialize, Serialize};

use crate::{
	models::{BasicModel, PrimaryId},
	utils, Address, IdPrefix,
};

#[derive(
	Clone, Debug, PartialEq, Eq, Serialize, Deserialize, DeriveEntityModel,
)]
#[sea_orm(table_name = "transactions")]
pub struct Model {
	#[sea_orm(primary_key)]
	#[serde(skip_deserializing)]
	pub transaction_id: PrimaryId,
	pub id: String,
	pub address: String,
	pub data: Json,
	#[sea_orm(nullable)]
	pub updated_at: Option<DateTime>,
	pub created_at: DateTime,
}

pub use ActiveModel as TransactionActiveModel;
pub use Model as Transaction;

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
	pub fn new_model(address: Address, data: Json) -> ActiveModel {
		ActiveModel {
			id: Set(utils::new_unique_id(IdPrefix::Transaction)),
			address: Set(address.to_string()),
			data: Set(data),
			..Default::default()
		}
	}
}
