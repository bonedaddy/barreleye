use axum::{
	extract::{Path, State},
	http::StatusCode,
	Json,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::{errors::ServerError, App, ServerResult};
use barreleye_common::models::{optional_set, ApiKey, ApiKeyActiveModel, BasicModel};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Payload {
	is_active: Option<bool>,
}

pub async fn handler(
	State(app): State<Arc<App>>,
	Path(api_key_id): Path<String>,
	Json(payload): Json<Payload>,
) -> ServerResult<StatusCode> {
	let update_data =
		ApiKeyActiveModel { is_active: optional_set(payload.is_active), ..Default::default() };

	if ApiKey::update_by_id(&app.db, &api_key_id, update_data).await? {
		Ok(StatusCode::NO_CONTENT)
	} else {
		Err(ServerError::NotFound)
	}
}
