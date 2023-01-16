use axum::{
	extract::{Query, State},
	Json,
};
use eyre::Result;
use serde::{Deserialize, Serialize};
use std::{
	collections::{HashMap, HashSet},
	sync::Arc,
};
use uuid::Uuid;

use crate::{App, ServerResult};
use barreleye_common::models::{Amount, Label, LabeledAddress, Link, Network, PrimaryId, Transfer};

#[derive(Deserialize)]
pub struct Payload {
	address: String,
	detailed: Option<bool>,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ResponseTransaction {
	hash: String,
	from_address: String,
	to_address: String,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ResponseUpstream {
	network: String,
	address: String,
	label: String,
	transactions: Vec<ResponseTransaction>,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Response {
	address: String,
	upstream: Vec<ResponseUpstream>,
	networks: Vec<Network>,
	labels: Vec<Label>,
}

pub async fn handler(
	State(app): State<Arc<App>>,
	Query(payload): Query<Payload>,
) -> ServerResult<Json<Response>> {
	let address = app.format_address(&payload.address).await?;

	// find links
	let links = match payload.detailed {
		Some(true) => Link::get_all_by_address(&app.warehouse, &address).await?,
		_ => Link::get_all_disinct_by_address(&app.warehouse, &address).await?,
	};

	// get transfers (@TODO ideally this step would be combined with link fetching)
	async fn get_transfers(app: Arc<App>, links: Vec<Link>) -> Result<HashMap<Uuid, Transfer>> {
		let transfer_uuids = {
			let mut ret = HashSet::new();

			for link in links.into_iter() {
				for transfer_uuid in link.transfer_uuids.into_iter() {
					ret.insert(transfer_uuid.0);
				}
			}

			ret
		};

		Ok(Transfer::get_all_by_uuids(&app.warehouse, transfer_uuids.into_iter().collect())
			.await?
			.into_iter()
			.map(|t| (t.uuid, t))
			.collect::<HashMap<Uuid, Transfer>>())
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

	// get labels data
	async fn get_labels_data(
		app: Arc<App>,
		addresses: Vec<String>,
	) -> Result<(HashMap<(PrimaryId, String), PrimaryId>, HashMap<PrimaryId, Label>)> {
		let mut labeled_address_map = HashMap::new();
		let mut labels = HashMap::new();

		let labeled_addresses =
			LabeledAddress::get_all_by_addresses(&app.db, addresses, Some(false)).await?;

		if !labeled_addresses.is_empty() {
			labeled_address_map = labeled_addresses
				.iter()
				.map(|a| ((a.network_id, a.address.clone()), a.label_id))
				.collect::<HashMap<(PrimaryId, String), PrimaryId>>();

			let mut label_ids =
				labeled_addresses.into_iter().map(|a| a.label_id).collect::<Vec<PrimaryId>>();

			label_ids.sort_unstable();
			label_ids.dedup();

			for label in Label::get_all_by_label_ids(&app.db, label_ids).await?.into_iter() {
				labels.insert(label.label_id, label);
			}
		}

		Ok((labeled_address_map, labels))
	}

	let mut labeled_addresses =
		links.iter().map(|l| l.from_address.clone()).collect::<Vec<String>>();

	labeled_addresses.sort_unstable();
	labeled_addresses.dedup();

	let (transfers, networks, labels_data) = tokio::join!(
		get_transfers(app.clone(), links.clone()),
		get_networks(app.clone(), &address),
		get_labels_data(app.clone(), labeled_addresses),
	);

	let transfers = transfers?;
	let networks = networks?;
	let (labeled_address_map, labels_map) = labels_data?;

	// assemble upstream
	let mut upstream = vec![];
	let n = app.networks.read().await;
	for link in links.into_iter() {
		let network_id = link.network_id as PrimaryId;
		if let Some(chain) = n.get(&network_id) {
			let network = chain.get_network();

			if let Some(&label_id) =
				labeled_address_map.get(&(network_id, link.from_address.clone()))
			{
				if let Some(label) = labels_map.get(&label_id) {
					upstream.push(ResponseUpstream {
						network: network.id,
						address: link.from_address,
						label: label.id.clone(),
						transactions: link
							.transfer_uuids
							.into_iter()
							.filter_map(|uuid| {
								transfers.get(&uuid.0).map(|t| ResponseTransaction {
									hash: t.tx_hash.clone(),
									from_address: t.from_address.clone(),
									to_address: t.to_address.clone(),
								})
							})
							.collect(),
					});
				}
			}
		}
	}

	Ok(Response { address, upstream, networks, labels: labels_map.into_values().collect() }.into())
}
