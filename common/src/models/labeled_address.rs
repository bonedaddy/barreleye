use eyre::Result;
use sea_orm::entity::{prelude::*, *};
use sea_orm_migration::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
	models::{label, BasicModel, PrimaryId, SoftDeleteModel},
	utils, Db, IdPrefix,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, DeriveEntityModel)]
#[sea_orm(table_name = "labeled_addresses")]
#[serde(rename_all = "camelCase")]
pub struct Model {
	#[sea_orm(primary_key)]
	#[serde(skip_serializing, skip_deserializing)]
	pub labeled_address_id: PrimaryId,
	#[serde(skip_serializing)]
	pub label_id: PrimaryId,
	#[serde(skip_serializing)]
	pub network_id: PrimaryId,
	pub id: String,
	pub address: String,
	pub description: String,
	#[serde(skip_serializing)]
	pub is_deleted: bool,
	#[sea_orm(nullable)]
	#[serde(skip_serializing)]
	pub updated_at: Option<DateTime>,
	pub created_at: DateTime,
}

pub use ActiveModel as LabeledAddressActiveModel;
pub use Model as LabeledAddress;

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {
	Label,
}

impl RelationTrait for Relation {
	fn def(&self) -> RelationDef {
		match self {
			Self::Label => Entity::belongs_to(label::Entity)
				.from(Column::LabelId)
				.to(label::Column::LabelId)
				.into(),
		}
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
	pub fn new_model(
		label_id: PrimaryId,
		network_id: PrimaryId,
		address: &str,
		description: &str,
	) -> ActiveModel {
		ActiveModel {
			label_id: Set(label_id),
			network_id: Set(network_id),
			id: Set(utils::new_unique_id(IdPrefix::LabeledAddress)),
			address: Set(address.to_string()),
			description: Set(description.to_string()),
			is_deleted: Set(false),
			..Default::default()
		}
	}

	pub async fn create_many(db: &Db, data: Vec<ActiveModel>) -> Result<PrimaryId> {
		let insert_result = Entity::insert_many(data)
			.on_conflict(
				OnConflict::columns([Column::NetworkId, Column::Address])
					// @TODO this should be a `.do_nothing()`, but: `https://github.com/SeaQL/sea-orm/issues/899`
					.update_column(Column::LabeledAddressId)
					.to_owned(),
			)
			.exec(db.get())
			.await?;

		Ok(insert_result.last_insert_id)
	}

	pub async fn get_all_by_label_ids(
		db: &Db,
		label_ids: Vec<PrimaryId>,
		is_deleted: Option<bool>,
	) -> Result<Vec<Self>> {
		let mut q = Entity::find().filter(Column::LabelId.is_in(label_ids));
		if is_deleted.is_some() {
			q = q.filter(Column::IsDeleted.eq(is_deleted.unwrap()))
		}

		Ok(q.all(db.get()).await?)
	}

	pub async fn get_by_address(
		db: &Db,
		address: &str,
		is_deleted: Option<bool>,
	) -> Result<Option<Self>> {
		let mut q = Entity::find().filter(Column::Address.eq(address));
		if is_deleted.is_some() {
			q = q.filter(Column::IsDeleted.eq(is_deleted.unwrap()))
		}

		Ok(q.one(db.get()).await?)
	}

	pub async fn get_all_by_network_ids(
		db: &Db,
		network_ids: Vec<PrimaryId>,
		is_deleted: Option<bool>,
	) -> Result<Vec<Self>> {
		let mut q = Entity::find().filter(Column::NetworkId.is_in(network_ids));
		if is_deleted.is_some() {
			q = q.filter(Column::IsDeleted.eq(is_deleted.unwrap()))
		}

		Ok(q.all(db.get()).await?)
	}

	pub async fn get_all_by_network_id_and_addresses(
		db: &Db,
		network_id: PrimaryId,
		addresses: Vec<String>,
		is_deleted: Option<bool>,
	) -> Result<Vec<Self>> {
		let mut q = Entity::find()
			.filter(Column::NetworkId.eq(network_id))
			.filter(Column::Address.is_in(addresses));

		if is_deleted.is_some() {
			q = q.filter(Column::IsDeleted.eq(is_deleted.unwrap()))
		}

		Ok(q.all(db.get()).await?)
	}

	pub async fn update_by_label_id(
		db: &Db,
		label_id: PrimaryId,
		data: ActiveModel,
	) -> Result<u64> {
		let res = Entity::update_many()
			.col_expr(Alias::new("updated_at"), Expr::value(utils::now()))
			.set(data)
			.filter(Column::LabelId.eq(label_id))
			.exec(db.get())
			.await?;

		Ok(res.rows_affected)
	}
}
