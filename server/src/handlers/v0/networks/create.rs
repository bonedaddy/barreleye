use axum::{extract::State, Json};
use serde::Deserialize;
use std::sync::Arc;

use crate::{errors::ServerError, ServerResult, ServerState};
use barreleye_common::{
	models::{BasicModel, Network},
	Blockchain, Env,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Payload {
	name: String,
	tag: String,
	env: Env,
	blockchain: Blockchain,
	chain_id: u64,
	expected_block_time: u16,
	rpc: String,
	rpc_bootstraps: Vec<String>,
}

pub async fn handler(
	State(app): State<Arc<ServerState>>,
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

	// create new
	let network_id = Network::create(
		&app.db,
		Network::new_model(
			&payload.name,
			&payload.tag,
			payload.env,
			payload.blockchain,
			payload.chain_id as i64,
			payload.expected_block_time as i16,
			&payload.rpc,
			payload.rpc_bootstraps,
		),
	)
	.await?;

	// return newly created
	Ok(Network::get(&app.db, network_id).await?.unwrap().into())
}
