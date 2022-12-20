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
use barreleye_common::models::{Amount, Network, PrimaryId};

#[derive(Deserialize)]
pub struct Payload {
	address: String,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ResponseAsset {
	network: String,
	address: String,
	amount: String,
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
	let mut response =
		Response { address: app.format_address(&payload.address).await?, ..Default::default() };

	let all_amounts = Amount::get_all_by_address(&app.warehouse, &payload.address).await?;
	if !all_amounts.is_empty() {
		let mut all_network_ids =
			all_amounts.iter().map(|a| a.network_id as PrimaryId).collect::<Vec<PrimaryId>>();
		all_network_ids.sort_unstable();
		all_network_ids.dedup();

		let networks: HashMap<u64, ResponseNetwork> =
			Network::get_all_by_network_ids(&app.db, all_network_ids)
				.await?
				.into_iter()
				.map(|n| (n.network_id as u64, ResponseNetwork { id: n.id, name: n.name }))
				.collect();

		for asset_amount in all_amounts.into_iter() {
			if asset_amount.amount.is_zero() {
				continue;
			}

			if let Some(network) = networks.get(&asset_amount.network_id) {
				let chain = app.networks.read().await;

				response.assets.push(ResponseAsset {
					network: network.id.clone(),
					address: chain[&(asset_amount.network_id as PrimaryId)]
						.format_address(&asset_amount.asset_address),
					amount: asset_amount.amount.to_string(),
				});

				let network = networks[&asset_amount.network_id].clone();
				response.networks.insert(network);
			}
		}
	}

	Ok(response.into())
}
