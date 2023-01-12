use axum::{extract::State, Json};
use serde::Deserialize;
use std::sync::Arc;

use crate::{errors::ServerError, App, ServerResult};
use barreleye_common::models::{BasicModel, Label};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Payload {
	name: String,
	description: String,
}

pub async fn handler(
	State(app): State<Arc<App>>,
	Json(payload): Json<Payload>,
) -> ServerResult<Json<Label>> {
	// check for duplicate name
	if Label::get_by_name(&app.db, &payload.name, None).await?.is_some() {
		return Err(ServerError::Duplicate { field: "name".to_string(), value: payload.name });
	}

	// create new
	let label_id =
		Label::create(&app.db, Label::new_model(&payload.name, &payload.description)).await?;

	// return newly created
	Ok(Label::get(&app.db, label_id).await?.unwrap().into())
}
