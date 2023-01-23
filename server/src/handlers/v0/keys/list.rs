use axum::{extract::State, Json};
use axum_extra::extract::Query;
use serde::Deserialize;
use std::sync::Arc;

use crate::{App, ServerResult};
use barreleye_common::models::{ApiKey, BasicModel};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Payload {
	offset: Option<u64>,
	limit: Option<u64>,
}

pub async fn handler(
	State(app): State<Arc<App>>,
	Query(payload): Query<Payload>,
) -> ServerResult<Json<Vec<ApiKey>>> {
	Ok(ApiKey::get_all_where(app.db(), vec![], payload.offset, payload.limit)
		.await?
		.iter()
		.map(|k| k.format())
		.collect::<Vec<ApiKey>>()
		.into())
}
