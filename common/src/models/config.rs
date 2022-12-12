use derive_more::Display;
use eyre::Result;
use regex::Regex;
use sea_orm::{entity::prelude::*, Condition, Set};
use sea_orm_migration::prelude::OnConflict;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;

use crate::{models::PrimaryId, utils, BlockHeight, Db};

#[derive(Display, Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum ConfigKey {
	#[display(fmt = "leader")]
	Leader,
	#[display(fmt = "label_fetched_l{}", "_0")]
	LabelFetched(PrimaryId),
	#[display(fmt = "indexer_tail_n{}_block", "_0")]
	IndexerTailBlock(PrimaryId),
	#[display(fmt = "indexer_tail_sync_n{}_b{}_blocks", "_0", "_1")]
	IndexerTailSyncBlocks(PrimaryId, BlockHeight),
	#[display(fmt = "indexer_head_n{}_m{}_blocks", "_0", "_1")]
	IndexerHeadBlocks(PrimaryId, u16),
	#[display(fmt = "indexer_n{}_m{}_synced", "_0", "_1")]
	IndexerSynced(PrimaryId, u16),
	#[display(fmt = "indexer_n{}_progress", "_0")]
	IndexerProgress(PrimaryId),
	#[display(fmt = "block_height_n{}", "_0")]
	BlockHeight(PrimaryId),
}

impl From<String> for ConfigKey {
	fn from(s: String) -> Self {
		let re = Regex::new(r"(\d+)").unwrap();

		let template = re.replace_all(&s, "{}");
		let n = re.find_iter(&s).filter_map(|n| n.as_str().parse().ok()).collect::<Vec<i64>>();

		match template.to_string().as_str() {
			"leader" => Self::Leader,
			"label_fetched_l{}" if n.len() == 1 => Self::LabelFetched(n[0]),
			"indexer_tail_n{}_block" if n.len() == 1 => Self::IndexerTailBlock(n[0]),
			"indexer_tail_sync_n{}_b{}_blocks" if n.len() == 2 => {
				Self::IndexerTailSyncBlocks(n[0], n[1] as BlockHeight)
			}
			"indexer_head_n{}_m{}_blocks" if n.len() == 2 => {
				Self::IndexerHeadBlocks(n[0], n[1] as u16)
			}
			"indexer_n{}_m{}_synced" if n.len() == 2 => Self::IndexerSynced(n[0], n[1] as u16),
			"indexer_n{}_progress" if n.len() == 1 => Self::IndexerProgress(n[0]),
			"block_height_n{}" if n.len() == 1 => Self::BlockHeight(n[0]),
			_ => panic!("no match in From<String> for ConfigKey"),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn config_key_from_to_str() {
		let config_keys = HashMap::from([
			(ConfigKey::Leader, "leader"),
			(ConfigKey::LabelFetched(123), "label_fetched_l123"),
			(ConfigKey::IndexerTailBlock(123), "indexer_tail_n123_block"),
			(ConfigKey::IndexerTailSyncBlocks(123, 456), "indexer_tail_sync_n123_b456_blocks"),
			(ConfigKey::IndexerHeadBlocks(123, 456), "indexer_head_n123_m456_blocks"),
			(ConfigKey::IndexerSynced(123, 456), "indexer_n123_m456_synced"),
			(ConfigKey::IndexerProgress(123), "indexer_n123_progress"),
			(ConfigKey::BlockHeight(123), "block_height_n123"),
		]);

		for (config_key, config_key_str) in config_keys.into_iter() {
			assert_eq!(config_key.to_string(), config_key_str);
			assert_eq!(Into::<ConfigKey>::into(config_key_str.to_string()), config_key);
		}
	}
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, DeriveEntityModel)]
#[sea_orm(table_name = "configs")]
#[serde(rename_all = "camelCase")]
pub struct Model {
	#[sea_orm(primary_key)]
	#[serde(skip_serializing, skip_deserializing)]
	pub config_id: PrimaryId,
	pub key: String,
	pub value: String,
	#[serde(skip_serializing)]
	pub updated_at: DateTime,
	pub created_at: DateTime,
}

#[derive(Debug)]
pub struct Value<T: for<'a> Deserialize<'a>> {
	pub value: T,
	pub updated_at: DateTime,
	pub created_at: DateTime,
}

pub use ActiveModel as ConfigActiveModel;
pub use Model as Config;

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {}

impl RelationTrait for Relation {
	fn def(&self) -> RelationDef {
		panic!("No RelationDef")
	}
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
	pub async fn set<T>(db: &Db, key: ConfigKey, value: T) -> Result<()>
	where
		T: Serialize,
	{
		Entity::insert(ActiveModel {
			key: Set(key.to_string()),
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

		Ok(())
	}

	pub async fn set_many<T>(db: &Db, values: HashMap<ConfigKey, T>) -> Result<()>
	where
		T: Serialize,
	{
		let insert_data = values
			.into_iter()
			.map(|(key, value)| ActiveModel {
				key: Set(key.to_string()),
				value: Set(json!(value).to_string()),
				updated_at: Set(utils::now()),
				..Default::default()
			})
			.collect::<Vec<ActiveModel>>();

		Entity::insert_many(insert_data)
			.on_conflict(
				OnConflict::column(Column::Key)
					.update_columns([Column::Value, Column::UpdatedAt])
					.to_owned(),
			)
			.exec(db.get())
			.await?;

		Ok(())
	}

	pub async fn get<T>(db: &Db, key: ConfigKey) -> Result<Option<Value<T>>>
	where
		T: for<'a> Deserialize<'a>,
	{
		Ok(Entity::find().filter(Column::Key.eq(key.to_string())).one(db.get()).await?.map(|m| {
			Value {
				value: serde_json::from_str(&m.value).unwrap(),
				updated_at: m.updated_at,
				created_at: m.created_at,
			}
		}))
	}

	pub async fn get_many<T>(db: &Db, keys: Vec<ConfigKey>) -> Result<HashMap<ConfigKey, Value<T>>>
	where
		T: for<'a> Deserialize<'a>,
	{
		Ok(Entity::find()
			.filter(Column::Key.is_in(keys.iter().map(|k| k.to_string())))
			.all(db.get())
			.await?
			.into_iter()
			.map(|m| {
				(
					m.key.into(),
					Value {
						value: serde_json::from_str(&m.value).unwrap(),
						updated_at: m.updated_at,
						created_at: m.created_at,
					},
				)
			})
			.collect())
	}

	pub async fn get_many_by_keyword<T>(
		db: &Db,
		keyword: &str,
	) -> Result<HashMap<ConfigKey, Value<T>>>
	where
		T: for<'a> Deserialize<'a>,
	{
		Ok(Entity::find()
			.filter(Self::get_keyword_condition(keyword))
			.all(db.get())
			.await?
			.into_iter()
			.map(|m| {
				(
					m.key.into(),
					Value {
						value: serde_json::from_str(&m.value).unwrap(),
						updated_at: m.updated_at,
						created_at: m.created_at,
					},
				)
			})
			.collect())
	}

	pub async fn delete(db: &Db, key: ConfigKey) -> Result<()> {
		Entity::delete_many().filter(Column::Key.eq(key.to_string())).exec(db.get()).await?;
		Ok(())
	}

	pub async fn delete_all_by_keyword(db: &Db, keyword: &str) -> Result<()> {
		Entity::delete_many().filter(Self::get_keyword_condition(keyword)).exec(db.get()).await?;

		Ok(())
	}

	fn get_keyword_condition(keyword: &str) -> Condition {
		Condition::any()
			.add(Column::Key.like(&format!("%_{keyword}_%")))
			.add(Column::Key.like(&format!("%_{keyword}")))
	}
}
