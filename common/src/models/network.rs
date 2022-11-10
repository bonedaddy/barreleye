use crate::Db;
use eyre::Result;
use sea_orm::{
	entity::prelude::*,
	sea_query::{func::Func, Expr},
	Condition, Set,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{
	models::{BasicModel, PrimaryId},
	utils, Blockchain, Env, IdPrefix,
};

#[derive(
	Clone, Debug, PartialEq, Eq, Serialize, Deserialize, DeriveEntityModel,
)]
#[sea_orm(table_name = "networks")]
#[serde(rename_all = "camelCase")]
pub struct Model {
	#[sea_orm(primary_key)]
	#[serde(skip_serializing, skip_deserializing)]
	pub network_id: PrimaryId,
	pub id: String,
	pub name: String,
	pub tag: String,
	pub env: Env,
	pub blockchain: Blockchain,
	pub chain_id: i64,
	pub expected_block_time: i16,
	pub rpc: String,
	pub rpc_bootstraps: Json,
	#[sea_orm(nullable)]
	#[serde(skip_serializing)]
	pub updated_at: Option<DateTime>,
	pub created_at: DateTime,
}

pub use ActiveModel as NetworkActiveModel;
pub use Model as Network;

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
		name: &str,
		tag: &str,
		env: Env,
		blockchain: Blockchain,
		chain_id: i64,
		expected_block_time: i16,
		rpc: &str,
		rpc_bootstraps: Vec<String>,
	) -> ActiveModel {
		ActiveModel {
			id: Set(utils::new_unique_id(IdPrefix::Network)),
			name: Set(name.to_string()),
			tag: Set(tag.to_string()),
			env: Set(env),
			blockchain: Set(blockchain),
			chain_id: Set(chain_id),
			expected_block_time: Set(expected_block_time),
			rpc: Set(rpc.to_string()),
			rpc_bootstraps: Set(json!(rpc_bootstraps)),
			..Default::default()
		}
	}

	pub async fn get_all_by_env(db: &Db, env: Env) -> Result<Vec<Self>> {
		Ok(Entity::find().filter(Column::Env.eq(env)).all(db.get()).await?)
	}

	pub async fn get_by_name(db: &Db, name: &str) -> Result<Option<Self>> {
		Ok(Entity::find()
			.filter(
				Condition::all().add(
					Func::lower(Expr::col(Column::Name))
						.equals(name.trim().to_lowercase()),
				),
			)
			.one(db.get())
			.await?)
	}

	pub async fn get_by_env_blockchain_and_chain_id(
		db: &Db,
		env: Env,
		blockchain: Blockchain,
		chain_id: i64,
	) -> Result<Option<Self>> {
		Ok(Entity::find()
			.filter(Column::Env.eq(env))
			.filter(Column::Blockchain.eq(blockchain))
			.filter(Column::ChainId.eq(chain_id))
			.one(db.get())
			.await?)
	}
}
