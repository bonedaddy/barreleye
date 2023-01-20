use axum::{extract::State, Json};
use serde::Deserialize;
use std::sync::Arc;

use crate::{App, ServerResult};
use barreleye_common::models::{BasicModel, Tag};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Payload {
	offset: Option<u64>,
	limit: Option<u64>,
}

pub async fn handler(
	State(app): State<Arc<App>>,
	Json(payload): Json<Option<Payload>>,
) -> ServerResult<Json<Vec<Tag>>> {
	let (offset, limit) = match payload {
		Some(v) => (v.offset, v.limit),
		_ => (None, None),
	};

	Ok(Tag::get_all_where(&app.db, vec![], offset, limit).await?.into())
}
