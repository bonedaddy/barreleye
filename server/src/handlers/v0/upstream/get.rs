use axum::{
	extract::{Query, State},
	Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{App, ServerResult};
use barreleye_common::{
	models::{BasicModel, Label, LabeledAddress},
	Risk,
};

#[derive(Deserialize)]
pub struct Payload {
	address: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Response {
	address: String,
	risk: Risk,
	label_ids: Vec<String>,
	labels: Vec<Label>,
}

pub async fn handler(
	State(app): State<Arc<App>>,
	Query(payload): Query<Payload>,
) -> ServerResult<Json<Response>> {
	let mut response = Response {
		address: payload.address.clone(),
		risk: Risk::Low,
		label_ids: vec![],
		labels: vec![],
	};

	if let Some(labeled_address) = LabeledAddress::get_by_address(&app.db, &payload.address).await?
	{
		let label = Label::get(&app.db, labeled_address.label_id).await?.unwrap();

		response.label_ids.push(label.id.clone());
		response.labels.push(label);
	}

	Ok(response.into())
}
