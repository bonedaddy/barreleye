use axum::{extract::State, Json};
use serde::Deserialize;
use std::sync::Arc;

use crate::{ServerResult, ServerState};
use barreleye_common::models::{BasicModel, Label};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Payload {
	offset: Option<u64>,
	limit: Option<u64>,
}

pub async fn handler(
	State(app): State<Arc<ServerState>>,
	Json(payload): Json<Option<Payload>>,
) -> ServerResult<Json<Vec<Label>>> {
	let (offset, limit) = match payload {
		Some(v) => (v.offset, v.limit),
		None => (None, None),
	};

	let labels = Label::get_all_where(&app.db, vec![], offset, limit).await?;

	Ok(labels.into())
}
