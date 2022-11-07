use axum::{
	extract::{Query, State},
	Json,
};
use serde::{Deserialize, Serialize};
use std::{str::FromStr, sync::Arc};

use crate::{ServerResult, ServerState};
use barreleye_common::{
	models::{BasicModel, Label, LabeledAddress},
	LabelId, Risk,
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
	State(app): State<Arc<ServerState>>,
	Query(payload): Query<Payload>,
) -> ServerResult<Json<Response>> {
	let mut response = Response {
		address: payload.address.clone(),
		risk: Risk::Low,
		label_ids: vec![],
		labels: vec![],
	};

	if let Some(labeled_address) =
		LabeledAddress::get_by_address(&app.db, &payload.address).await?
	{
		let label =
			Label::get(&app.db, labeled_address.label_id).await?.unwrap();

		match LabelId::from_str(&label.id) {
			Ok(LabelId::Ofac) => response.risk = Risk::Severe,
			Ok(LabelId::Ofsi) => response.risk = Risk::Severe,
			_ => {}
		}

		response.label_ids.push(label.id.clone());
		response.labels.push(label);
	}

	Ok(response.into())
}
