use axum::{extract::State, Json};
use serde::Deserialize;
use std::sync::Arc;

use crate::{ServerResult, ServerState};
use barreleye_common::models::{ApiKey, BasicModel};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Payload {
	offset: Option<u64>,
	limit: Option<u64>,
}

pub async fn handler(
	State(app): State<Arc<ServerState>>,
	Json(payload): Json<Option<Payload>>,
) -> ServerResult<Json<Vec<ApiKey>>> {
	let (offset, limit) = match payload {
		Some(v) => (v.offset, v.limit),
		_ => (None, None),
	};

	Ok(ApiKey::get_all_where(&app.db, vec![], offset, limit)
		.await?
		.iter()
		.map(|ak| ak.format())
		.collect::<Vec<ApiKey>>()
		.into())
}
