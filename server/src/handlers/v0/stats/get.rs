use axum::{extract::State, Json};
use serde::Serialize;
use std::sync::Arc;

use crate::{AppState, ServerResult};
use barreleye_common::models::{BasicModel, Config, ConfigKey, Network};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseNetwork {
	name: String,
	block_index: u64,
	block_height: u64,
	sync: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Response {
	sync: f64,
	networks: Vec<ResponseNetwork>,
}

pub async fn handler(State(app): State<Arc<AppState>>) -> ServerResult<Json<Response>> {
	let all_networks = Network::get_all(&app.db).await?;

	let all_configs = {
		let mut all_cache_keys = vec![];

		for network in all_networks.iter() {
			let nid = network.network_id as u64;

			all_cache_keys.push(ConfigKey::IndexerTailBlock(nid));
			all_cache_keys.push(ConfigKey::BlockHeight(nid));
		}

		Config::get_many::<u64>(&app.db, all_cache_keys).await?
	};

	let mut networks = vec![];
	for n in all_networks.into_iter() {
		let block_height = {
			let cache_key = ConfigKey::BlockHeight(n.network_id as u64).to_string();
			match all_configs.contains_key(&cache_key) {
				true => all_configs[&cache_key].value,
				_ => 0,
			}
		};

		let block_index = {
			let cache_key = ConfigKey::IndexerTailBlock(n.network_id as u64).to_string();
			match all_configs.contains_key(&cache_key) {
				true => all_configs[&cache_key].value,
				_ => 0,
			}
		};

		let sync = match block_height > 0 {
			true => block_index as f64 / block_height as f64,
			_ => 0.0,
		};

		networks.push(ResponseNetwork { name: n.name, block_index, block_height, sync });
	}

	Ok(Response {
		sync: networks.iter().map(|n| n.sync).sum::<f64>() / networks.len() as f64,
		networks,
	}
	.into())
}
