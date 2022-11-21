use axum::{extract::State, Json};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

use crate::{errors::ServerError, AppState, ServerResult};
use barreleye_chain::{Bitcoin, ChainTrait, Evm};
use barreleye_common::{
	models::{BasicModel, Network},
	utils, Blockchain, Env,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Payload {
	name: String,
	tag: String,
	env: Env,
	blockchain: Blockchain,
	chain_id: u64,
	block_time_ms: u64,
	rpc: String,
	rpc_bootstraps: Vec<String>,
}

pub async fn handler(
	State(app): State<Arc<AppState>>,
	Json(payload): Json<Payload>,
) -> ServerResult<Json<Network>> {
	// check for duplicate name
	if Network::get_by_name(&app.db, &payload.name).await?.is_some() {
		return Err(ServerError::Duplicate {
			field: "name".to_string(),
			value: payload.name,
		});
	}

	// check for duplicate chain id
	if Network::get_by_env_blockchain_and_chain_id(
		&app.db,
		payload.env,
		payload.blockchain,
		payload.chain_id as i64,
	)
	.await?
	.is_some()
	{
		return Err(ServerError::Duplicate {
			field: "chain_id".to_string(),
			value: payload.chain_id.to_string(),
		});
	}

	// check rpc connection
	let n = Network {
		network_id: 0,
		id: "".to_string(),
		name: payload.name.clone(),
		tag: payload.tag.clone(),
		env: payload.env,
		blockchain: payload.blockchain,
		chain_id: payload.chain_id as i64,
		block_time_ms: 0,
		rpc: payload.rpc.clone(),
		rpc_bootstraps: json!(payload.rpc_bootstraps.clone()),
		updated_at: None,
		created_at: utils::now(),
	};
	let service_name = n.name.clone();
	let a = app.clone();
	let _: Box<dyn ChainTrait> = match payload.blockchain {
		Blockchain::Bitcoin => {
			Box::new(Bitcoin::new(a, n, None).await.map_err(|_| {
				ServerError::InvalidService { name: service_name.clone() }
			})?)
		}
		Blockchain::Evm => {
			Box::new(Evm::new(a, n, None).await.map_err(|_| {
				ServerError::InvalidService { name: service_name.clone() }
			})?)
		}
	};

	// create new
	let network_id = Network::create(
		&app.db,
		Network::new_model(
			&payload.name,
			&payload.tag,
			payload.env,
			payload.blockchain,
			payload.chain_id as i64,
			payload.block_time_ms as i64,
			&payload.rpc,
			payload.rpc_bootstraps,
		),
	)
	.await?;

	// return newly created
	Ok(Network::get(&app.db, network_id).await?.unwrap().into())
}
