use axum::{extract::State, Json};
use axum_extra::extract::Query;
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
	Query(payload): Query<Payload>,
) -> ServerResult<Json<Vec<Tag>>> {
	Ok(Tag::get_all_where(app.db(), vec![], payload.offset, payload.limit).await?.into())
}
