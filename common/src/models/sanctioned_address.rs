use eyre::Result;
use sea_orm::entity::{prelude::*, *};
use sea_orm_migration::prelude::OnConflict;
use serde::{Deserialize, Serialize};

use crate::{models::BasicModel, utils, IdPrefix};

#[derive(Debug, Serialize, Deserialize)]
pub enum Status {
	#[serde(rename = "NO_ISSUES_FOUND")]
	NoIssuesFound,
	#[serde(rename = "SANCTIONED")]
	Sanctioned,
	#[serde(rename = "DOWNSTREAM_OF_SANCTIONED")]
	DownstreamOfSanctioned,
}

#[derive(
	Clone, Debug, PartialEq, Eq, Serialize, Deserialize, DeriveEntityModel,
)]
#[sea_orm(table_name = "sanctioned_addresses")]
pub struct Model {
	#[sea_orm(primary_key)]
	#[serde(skip_deserializing)]
	pub sanctioned_address_id: i64,
	pub id: String,
	pub source: String,
	pub address: String,
	pub symbol: String,
	#[sea_orm(nullable)]
	pub updated_at: Option<DateTime>,
	pub created_at: DateTime,
}

pub use ActiveModel as SanctionedAddressActiveModel;
pub use Model as SanctionedAddress;

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
	pub fn new_model(
		source: &str,
		address: &str,
		symbol: &str,
	) -> Result<ActiveModel> {
		Ok(ActiveModel {
			id: Set(utils::new_unique_id(IdPrefix::SanctionedAddress)),
			source: Set(source.to_string()),
			address: Set(address.to_string()),
			symbol: Set(symbol.to_string()),
			..Default::default()
		})
	}

	pub async fn try_create(
		db: &DatabaseConnection,
		active_model: ActiveModel,
	) -> Result<i64> {
		let res = Entity::insert(active_model)
			.on_conflict(
				OnConflict::columns([Column::Source, Column::Address])
					/*
						@TODO
						- bug: `https://github.com/SeaQL/sea-orm/issues/899`
						- temporary fix: `https://github.com/SeaQL/sea-orm/issues/899#issuecomment-1204732330`
						- ideally: `.do_nothing()`
					*/
					.update_column(Column::Address)
					.to_owned(),
			)
			.exec(db)
			.await?;

		Ok(res.last_insert_id)
	}

	pub async fn get_by_address(
		db: &DatabaseConnection,
		address: &str,
	) -> Result<Option<Self>> {
		let sanctioned_address = Entity::find()
			.filter(Column::Address.eq(address.to_lowercase()))
			.one(db)
			.await?;

		Ok(sanctioned_address)
	}
}
