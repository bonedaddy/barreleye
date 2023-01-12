use axum::{extract::State, Json};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

use crate::{errors::ServerError, App, ServerResult};
use barreleye_common::{
	chain::{Bitcoin, ChainTrait, Evm},
	models::{BasicModel, Config, ConfigKey, Network},
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
	rps: u32,
}

pub async fn handler(
	State(app): State<Arc<App>>,
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
			field: "chainId".to_string(),
			value: payload.chain_id.to_string(),
		});
	}

	// check rpc connection
	let c = app.cache.clone();
	let n = Network { rpc_endpoints: json!(payload.rpc_endpoints.clone()), ..Default::default() };
	let mut boxed_chain: Box<dyn ChainTrait> = match payload.blockchain {
		Blockchain::Bitcoin => Box::new(Bitcoin::new(c, n)),
		Blockchain::Evm => Box::new(Evm::new(c, n)),
	};
	if !boxed_chain.connect().await? {
		return Err(ServerError::InvalidService { name: boxed_chain.get_network().name });
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
			payload.block_time_ms as i64,
			payload.rpc_endpoints,
			payload.rps as i32,
		),
	)
	.await?;

	// update config
	Config::set::<u8>(&app.db, ConfigKey::NetworksUpdated, 1).await?;

	// update app's networks
	let mut networks = app.networks.write().await;
	*networks = app.get_networks().await?;

	// return newly created
	Ok(Network::get(&app.db, network_id).await?.unwrap().into())
}
