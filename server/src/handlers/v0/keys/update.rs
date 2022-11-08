use axum::{
	extract::{Path, State},
	http::StatusCode,
	Json,
};
use sea_orm::entity::ActiveValue;
use serde::Deserialize;
use std::sync::Arc;

use crate::{errors::ServerError, ServerResult, ServerState};
use barreleye_common::models::{ApiKey, ApiKeyActiveModel, BasicModel};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Payload {
	is_admin: Option<bool>,
}

pub async fn handler(
	State(app): State<Arc<ServerState>>,
	Path(api_key_id): Path<String>,
	Json(payload): Json<Payload>,
) -> ServerResult<StatusCode> {
	let update_data = ApiKeyActiveModel {
		is_admin: match payload.is_admin {
			Some(is_admin) => ActiveValue::set(is_admin),
			_ => ActiveValue::not_set(),
		},
		..Default::default()
	};

	if ApiKey::update_by_id(&app.db, &api_key_id, update_data).await? {
		Ok(StatusCode::NO_CONTENT)
	} else {
		Err(ServerError::NotFound)
	}
}
