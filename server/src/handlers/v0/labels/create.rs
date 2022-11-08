use axum::{extract::State, Json};
use serde::Deserialize;
use std::sync::Arc;

use crate::{errors::ServerError, ServerResult, ServerState};
use barreleye_common::models::{BasicModel, Label};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Payload {
	name: String,
	is_tracked: bool,
}

pub async fn handler(
	State(app): State<Arc<ServerState>>,
	Json(payload): Json<Payload>,
) -> ServerResult<Json<Label>> {
	// check for duplicates
	if Label::get_by_name(&app.db, &payload.name).await?.is_some() {
		return Err(ServerError::Duplicate {
			field: "name".to_string(),
			value: payload.name,
		});
	}

	// create a label
	let label_id = Label::create(
		&app.db,
		Label::new_model(payload.name, true, false, payload.is_tracked),
	)
	.await?;

	// return newly created
	let label = Label::get(&app.db, label_id).await?.unwrap();
	Ok(label.into())
}
