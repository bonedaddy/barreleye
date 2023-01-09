use axum::{
	extract::{Query, State},
	Json,
};
use serde::{Deserialize, Serialize};
use std::{
	collections::{HashMap, HashSet},
	sync::Arc,
};

use crate::{App, ServerResult};
use barreleye_common::models::{Balance, Network, PrimaryId};

#[derive(Deserialize)]
pub struct Payload {
	address: String,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ResponseAsset {
	network: String,
	address: String,
	balance: String,
}

#[derive(Serialize, Default, Eq, PartialEq, Hash, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ResponseNetwork {
	id: String,
	name: String,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Response {
	address: String,
	assets: Vec<ResponseAsset>,
	networks: HashSet<ResponseNetwork>,
}

pub async fn handler(
	State(app): State<Arc<App>>,
	Query(payload): Query<Payload>,
) -> ServerResult<Json<Response>> {
	let address = app.format_address(&payload.address).await?;
	let mut response = Response { address: address.clone(), ..Default::default() };

	let all_balances = Balance::get_all_by_address(&app.warehouse, &address).await?;
	if !all_balances.is_empty() {
		let mut all_network_ids =
			all_balances.iter().map(|a| a.network_id as PrimaryId).collect::<Vec<PrimaryId>>();

		all_network_ids.sort_unstable();
		all_network_ids.dedup();

		let networks: HashMap<u64, ResponseNetwork> =
			Network::get_all_by_network_ids(&app.db, all_network_ids)
				.await?
				.into_iter()
				.map(|n| (n.network_id as u64, ResponseNetwork { id: n.id, name: n.name }))
				.collect();

		for balance_data in all_balances.into_iter() {
			if balance_data.balance.is_zero() {
				continue;
			}

			if let Some(network) = networks.get(&balance_data.network_id) {
				let n = app.networks.read().await;
				let network_id = balance_data.network_id as PrimaryId;

				if n.contains_key(&network_id) {
					response.assets.push(ResponseAsset {
						network: network.id.clone(),
						address: if balance_data.asset_address.is_empty() {
							"".to_string()
						} else {
							n[&network_id].format_address(&balance_data.asset_address)
						},
						balance: balance_data.balance.to_string(),
					});

					let network = networks[&balance_data.network_id].clone();
					response.networks.insert(network);
				}
			}
		}
	}

	Ok(response.into())
}
