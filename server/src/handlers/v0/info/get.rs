use axum::{
	extract::{Query, State},
	Json,
};
use eyre::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{App, ServerResult};
use barreleye_common::models::{Address, Amount, Balance, Entity, Network, PrimaryId};

#[derive(Deserialize)]
pub struct Payload {
	address: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseAsset {
	network: String,
	address: Option<String>,
	balance: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Response {
	address: String,
	assets: Vec<ResponseAsset>,
	networks: Vec<Network>,
	entities: Vec<Entity>,
}

pub async fn handler(
	State(app): State<Arc<App>>,
	Query(payload): Query<Payload>,
) -> ServerResult<Json<Response>> {
	let address = app.format_address(&payload.address).await?;

	// get assets
	async fn get_assets(app: Arc<App>, address: &str) -> Result<Vec<ResponseAsset>> {
		let mut ret = vec![];

		let n = app.networks.read().await;
		let all_balances = Balance::get_all_by_address(&app.warehouse, address).await?;
		if !all_balances.is_empty() {
			for balance_data in all_balances.into_iter() {
				if balance_data.balance.is_zero() {
					continue;
				}

				let network_id = balance_data.network_id as PrimaryId;
				if let Some(chain) = n.get(&network_id) {
					ret.push(ResponseAsset {
						network: chain.get_network().id,
						address: if balance_data.asset_address.is_empty() {
							None
						} else {
							Some(chain.format_address(&balance_data.asset_address))
						},
						balance: balance_data.balance.to_string(),
					});
				}
			}
		}

		Ok(ret)
	}

	// get networks
	async fn get_networks(app: Arc<App>, address: &str) -> Result<Vec<Network>> {
		let mut ret = vec![];

		let n = app.networks.read().await;
		let network_ids = Amount::get_all_network_ids_by_address(&app.warehouse, address).await?;
		if !network_ids.is_empty() {
			for (_, chain) in n.iter().filter(|(network_id, _)| network_ids.contains(network_id)) {
				ret.push(chain.get_network());
			}
		}

		Ok(ret)
	}

	// get entities
	async fn get_entities(app: Arc<App>, address: &str) -> Result<Vec<Entity>> {
		let mut ret = vec![];

		let addresses =
			Address::get_all_by_addresses(&app.db, vec![address.to_string()], Some(false)).await?;
		if !addresses.is_empty() {
			let mut entity_ids =
				addresses.into_iter().map(|a| a.entity_id).collect::<Vec<PrimaryId>>();

			entity_ids.sort_unstable();
			entity_ids.dedup();

			for entity in Entity::get_all_by_entity_ids(&app.db, entity_ids).await?.into_iter() {
				ret.push(entity);
			}
		}

		Ok(ret)
	}

	let (assets, networks, entities) = tokio::join!(
		get_assets(app.clone(), &address),
		get_networks(app.clone(), &address),
		get_entities(app.clone(), &address),
	);

	Ok(Response {
		address: address.clone(),
		assets: assets?,
		networks: networks?,
		entities: entities?,
	}
	.into())
}
