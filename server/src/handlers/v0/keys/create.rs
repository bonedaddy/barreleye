use axum::{extract::State, Json};
use serde::Deserialize;
use std::sync::Arc;

use crate::ServerResult;
use barreleye_common::{
	models::{ApiKey, BasicModel},
	App,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Payload {
	is_admin: Option<bool>,
}

pub async fn handler(
	State(app): State<Arc<App>>,
	Json(payload): Json<Payload>,
) -> ServerResult<Json<ApiKey>> {
	// create new
	let api_key_id =
		ApiKey::create(app.db(), ApiKey::new_model(payload.is_admin.unwrap_or(false))).await?;

	// return newly created
	Ok(ApiKey::get(app.db(), api_key_id).await?.unwrap().format().into())
}
