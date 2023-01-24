use eyre::Result;
use sea_orm::{
	entity::prelude::*,
	sea_query::{func::Func, Expr},
	Condition, ConnectionTrait, Set,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{
	models::{BasicModel, PrimaryId},
	utils, Blockchain, Env, IdPrefix,
};

#[derive(Default, Clone, Debug, PartialEq, Eq, Serialize, Deserialize, DeriveEntityModel)]
#[sea_orm(table_name = "networks")]
#[serde(rename_all = "camelCase")]
pub struct Model {
	#[sea_orm(primary_key)]
	#[serde(skip_serializing, skip_deserializing)]
	pub network_id: PrimaryId,
	pub id: String,
	pub name: String,
	pub env: Env,
	pub blockchain: Blockchain,
	pub chain_id: i64,
	pub block_time_ms: i64,
	pub rpc_endpoints: Json,
	pub rps: i32,
	pub is_active: bool,
	#[sea_orm(nullable)]
	#[serde(skip_serializing)]
	pub updated_at: Option<DateTime>,
	pub created_at: DateTime,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SanitizedNetwork {
	pub id: String,
	pub name: String,
	pub env: Env,
	pub chain_id: i64,
}

impl From<Model> for SanitizedNetwork {
	fn from(m: Model) -> SanitizedNetwork {
		SanitizedNetwork { id: m.id, name: m.name, env: m.env, chain_id: m.chain_id }
	}
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
		env: Env,
		blockchain: Blockchain,
		chain_id: i64,
		block_time_ms: i64,
		rpc_endpoints: Vec<String>,
		rps: i32,
	) -> ActiveModel {
		ActiveModel {
			id: Set(utils::new_unique_id(IdPrefix::Network)),
			name: Set(name.to_string()),
			env: Set(env),
			blockchain: Set(blockchain),
			chain_id: Set(chain_id),
			block_time_ms: Set(block_time_ms),
			rpc_endpoints: Set(json!(rpc_endpoints)),
			is_active: Set(true),
			rps: Set(rps),
			..Default::default()
		}
	}

	pub async fn get_all_by_env<C>(c: &C, env: Env) -> Result<Vec<Self>>
	where
		C: ConnectionTrait,
	{
		Ok(Entity::find().filter(Column::Env.eq(env)).all(c).await?)
	}

	pub async fn get_all_by_network_ids<C>(
		c: &C,
		mut network_ids: Vec<PrimaryId>,
	) -> Result<Vec<Self>>
	where
		C: ConnectionTrait,
	{
		network_ids.sort_unstable();
		network_ids.dedup();

		Ok(Entity::find().filter(Column::NetworkId.is_in(network_ids)).all(c).await?)
	}

	pub async fn get_by_name<C>(c: &C, name: &str) -> Result<Option<Self>>
	where
		C: ConnectionTrait,
	{
		Ok(Entity::find()
			.filter(
				Condition::all()
					.add(Func::lower(Expr::col(Column::Name)).equals(name.trim().to_lowercase())),
			)
			.one(c)
			.await?)
	}

	pub async fn get_by_env_blockchain_and_chain_id<C>(
		c: &C,
		env: Env,
		blockchain: Blockchain,
		chain_id: i64,
	) -> Result<Option<Self>>
	where
		C: ConnectionTrait,
	{
		Ok(Entity::find()
			.filter(Column::Env.eq(env))
			.filter(Column::Blockchain.eq(blockchain))
			.filter(Column::ChainId.eq(chain_id))
			.one(c)
			.await?)
	}
}
