use derive_more::Display;
use eyre::Result;
use sea_orm::{entity::prelude::*, Set};
use sea_orm_migration::prelude::OnConflict;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{models::PrimaryId, utils, Db};

#[derive(Display)]
pub enum CacheKey {
	#[display(fmt = "leader")]
	Leader,
	#[display(fmt = "label_fetched_{}", "_0")]
	LabelFetched(PrimaryId),
	#[display(fmt = "last_saved_block_{}", "_0")]
	LastSavedBlock(u64),
}

impl From<CacheKey> for String {
	fn from(cache_key: CacheKey) -> String {
		cache_key.to_string()
	}
}

#[derive(
	Clone, Debug, PartialEq, Eq, Serialize, Deserialize, DeriveEntityModel,
)]
#[sea_orm(table_name = "cache")]
#[serde(rename_all = "camelCase")]
pub struct Model {
	#[sea_orm(primary_key)]
	#[serde(skip_serializing, skip_deserializing)]
	pub cache_id: PrimaryId,
	pub key: String,
	pub value: String,
	#[serde(skip_serializing)]
	pub updated_at: DateTime,
	pub created_at: DateTime,
}

pub struct Value<T: for<'a> Deserialize<'a>> {
	pub value: T,
	pub updated_at: DateTime,
	pub created_at: DateTime,
}

pub use ActiveModel as CacheActiveModel;
pub use Model as Cache;

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {}

impl RelationTrait for Relation {
	fn def(&self) -> RelationDef {
		panic!("No RelationDef")
	}
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
	pub async fn set<T>(db: &Db, key: String, value: T) -> Result<PrimaryId>
	where
		T: Serialize,
	{
		let insert_result = Entity::insert(ActiveModel {
			key: Set(key),
			value: Set(json!(value).to_string()),
			updated_at: Set(utils::now()),
			..Default::default()
		})
		.on_conflict(
			OnConflict::column(Column::Key)
				.update_columns([Column::Value, Column::UpdatedAt])
				.to_owned(),
		)
		.exec(db.get())
		.await?;

		Ok(insert_result.last_insert_id)
	}

	pub async fn get<T>(db: &Db, key: String) -> Result<Option<Value<T>>>
	where
		T: for<'a> Deserialize<'a>,
	{
		Ok(Entity::find().filter(Column::Key.eq(key)).one(db.get()).await?.map(
			|m| Value {
				value: serde_json::from_str(&m.value).unwrap(),
				updated_at: m.updated_at,
				created_at: m.created_at,
			},
		))
	}
}
