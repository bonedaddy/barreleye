use axum::{extract::State, Json};
use serde::Deserialize;
use std::{collections::HashMap, sync::Arc};

use crate::{errors::ServerError, App, ServerResult};
use barreleye_common::models::{BasicModel, Label, LabeledAddress, Network};

type Address = String;
type Description = String;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Payload {
	label: String,
	network: String,
	addresses: HashMap<Address, Description>,
}

pub async fn handler(
	State(app): State<Arc<App>>,
	Json(payload): Json<Payload>,
) -> ServerResult<Json<Vec<LabeledAddress>>> {
	let label = Label::get_by_id(&app.db, &payload.label)
		.await?
		.ok_or(ServerError::InvalidParam { field: "label".to_string(), value: payload.label })?;

	let network =
		Network::get_by_id(&app.db, &payload.network).await?.ok_or(ServerError::InvalidParam {
			field: "network".to_string(),
			value: payload.network,
		})?;

	// check for duplicates
	let labeled_addresses = LabeledAddress::get_all_by_network_id_and_addresses(
		&app.db,
		network.network_id,
		payload.addresses.clone().into_keys().collect(),
	)
	.await?;
	if !labeled_addresses.is_empty() {
		return Err(ServerError::Duplicate {
			field: "addresses".to_string(),
			value: labeled_addresses[0].address.clone(),
		});
	}

	// create new
	LabeledAddress::create_many(
		&app.db,
		payload
			.addresses
			.clone()
			.iter()
			.map(|(address, description)| {
				LabeledAddress::new_model(label.label_id, network.network_id, address, description)
			})
			.collect(),
	)
	.await?;

	// return newly created
	Ok(LabeledAddress::get_all_by_network_id_and_addresses(
		&app.db,
		network.network_id,
		payload.addresses.into_keys().collect(),
	)
	.await?
	.into())
}
