use axum::{
	extract::{Path, State},
	http::StatusCode,
	Json,
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

use crate::{errors::ServerError, AppState, ServerResult};
use barreleye_common::{
	models::{optional_set, BasicModel, Network, NetworkActiveModel},
	Blockchain, Env,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Payload {
	name: Option<String>,
	tag: Option<String>,
	env: Option<Env>,
	blockchain: Option<Blockchain>,
	chain_id: Option<u64>,
	expected_block_time: Option<u16>,
	rpc: Option<String>,
	rpc_bootstraps: Option<Vec<String>>,
}

pub async fn handler(
	State(app): State<Arc<AppState>>,
	Path(network_id): Path<String>,
	Json(payload): Json<Payload>,
) -> ServerResult<StatusCode> {
	let network = Network::get_by_id(&app.db, &network_id)
		.await?
		.ok_or(ServerError::NotFound)?;

	// check for duplicate name
	if let Some(name) = payload.name.clone() {
		if network_id != network.id &&
			network.name.trim().to_lowercase() == name.trim().to_lowercase()
		{
			return Err(ServerError::Duplicate {
				field: "name".to_string(),
				value: name,
			});
		}
	}

	// check for duplicate chain id
	if let Some(chain_id) = payload.chain_id {
		if Network::get_by_env_blockchain_and_chain_id(
			&app.db,
			payload.env.unwrap_or(network.env),
			payload.blockchain.unwrap_or(network.blockchain),
			chain_id as i64,
		)
		.await?
		.is_some()
		{
			return Err(ServerError::Duplicate {
				field: "chain_id".to_string(),
				value: chain_id.to_string(),
			});
		}
	}

	let update_data = NetworkActiveModel {
		name: optional_set(payload.name),
		tag: optional_set(payload.tag),
		env: optional_set(payload.env),
		blockchain: optional_set(payload.blockchain),
		chain_id: optional_set(payload.chain_id.map(|v| v as i64)),
		expected_block_time: optional_set(
			payload.expected_block_time.map(|v| v as i16),
		),
		rpc: optional_set(payload.rpc),
		rpc_bootstraps: optional_set(payload.rpc_bootstraps.map(|v| json!(v))),
		..Default::default()
	};

	if Network::update_by_id(&app.db, &network_id, update_data).await? {
		Ok(StatusCode::NO_CONTENT)
	} else {
		Err(ServerError::NotFound)
	}
}
