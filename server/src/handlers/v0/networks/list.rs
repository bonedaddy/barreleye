use axum::{extract::State, Json};
use axum_extra::extract::Query;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::ServerResult;
use barreleye_common::{
	models::{BasicModel, Network},
	App,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Payload {
	offset: Option<u64>,
	limit: Option<u64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Response {
	networks: Vec<Network>,
}

pub async fn handler(
	State(app): State<Arc<App>>,
	Query(payload): Query<Payload>,
) -> ServerResult<Json<Response>> {
	let networks = Network::get_all_where(app.db(), vec![], payload.offset, payload.limit).await?;
	Ok(Response { networks }.into())
}
