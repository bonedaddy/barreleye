use axum::{
	extract::{Query, State},
	Json,
};
use eyre::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{App, ServerResult};
use barreleye_common::models::{Amount, Balance, Label, LabeledAddress, Network, PrimaryId};

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

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Response {
	address: String,
	assets: Vec<ResponseAsset>,
	networks: Vec<Network>,
	labels: Vec<Label>,
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
							"".to_string()
						} else {
							chain.format_address(&balance_data.asset_address)
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

	// get labels
	async fn get_labels(app: Arc<App>, address: &str) -> Result<Vec<Label>> {
		let mut ret = vec![];

		let labeled_addresses =
			LabeledAddress::get_all_by_addresses(&app.db, vec![address.to_string()], Some(false))
				.await?;
		if !labeled_addresses.is_empty() {
			let mut label_ids =
				labeled_addresses.into_iter().map(|a| a.label_id).collect::<Vec<PrimaryId>>();

			label_ids.sort_unstable();
			label_ids.dedup();

			for label in Label::get_all_by_label_ids(&app.db, label_ids).await?.into_iter() {
				ret.push(label);
			}
		}

		Ok(ret)
	}

	let (assets, networks, labels) = tokio::join!(
		get_assets(app.clone(), &address),
		get_networks(app.clone(), &address),
		get_labels(app.clone(), &address),
	);

	Ok(Response { address: address.clone(), assets: assets?, networks: networks?, labels: labels? }
		.into())
}
