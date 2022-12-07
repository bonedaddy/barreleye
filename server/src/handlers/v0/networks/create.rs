use axum::{extract::State, Json};
use sea_orm::TryIntoModel;
use serde::Deserialize;
use std::sync::Arc;

use crate::{errors::ServerError, AppState, ServerResult};
use barreleye_chain::{Bitcoin, ChainTrait, Evm};
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
	block_time_ms: u64,
	rpc_endpoints: Vec<String>,
}

pub async fn handler(
	State(app): State<Arc<AppState>>,
	Json(payload): Json<Payload>,
) -> ServerResult<Json<Network>> {
	// check for duplicate name
	if Network::get_by_name(&app.db, &payload.name).await?.is_some() {
		return Err(ServerError::Duplicate { field: "name".to_string(), value: payload.name });
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
	let n = Network::new_model(
		&payload.name.clone(),
		&payload.tag.clone(),
		payload.env,
		payload.blockchain,
		payload.chain_id as i64,
		0,
		payload.rpc_endpoints.clone(),
	)
	.try_into_model()?;
	let _: Box<dyn ChainTrait> = match payload.blockchain {
		Blockchain::Bitcoin => Box::new(
			Bitcoin::new(app.clone(), n.clone(), None)
				.await
				.map_err(|_| ServerError::InvalidService { name: n.name })?,
		),
		Blockchain::Evm => Box::new(
			Evm::new(app.clone(), n.clone(), None)
				.await
				.map_err(|_| ServerError::InvalidService { name: n.name })?,
		),
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
			payload.rpc_endpoints,
		),
	)
	.await?;

	// return newly created
	Ok(Network::get(&app.db, network_id).await?.unwrap().into())
}
