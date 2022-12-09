use axum::{extract::State, Json};
use serde::Deserialize;
use std::sync::Arc;

use crate::{errors::ServerError, AppState, ServerResult};
use barreleye_common::{
	models::{BasicModel, Label, LabeledAddress},
	Address,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Payload {
	label: String,
	addresses: Vec<String>,
}

pub async fn handler(
	State(app): State<Arc<AppState>>,
	Json(payload): Json<Payload>,
) -> ServerResult<Json<Vec<LabeledAddress>>> {
	let label = Label::get_by_id(&app.db, &payload.label)
		.await?
		.ok_or(ServerError::InvalidParam { field: "label".to_string(), value: payload.label })?;

	// check for duplicates
	let labeled_addresses = LabeledAddress::get_all_by_label_id_and_addresses(
		&app.db,
		label.label_id,
		payload.addresses.clone(),
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
			.map(|a| LabeledAddress::new_model(label.label_id, Address::new(a)))
			.collect(),
	)
	.await?;

	// return newly created
	Ok(LabeledAddress::get_all_by_label_id_and_addresses(
		&app.db,
		label.label_id,
		payload.addresses,
	)
	.await?
	.into())
}
