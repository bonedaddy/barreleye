use axum::{
	extract::{Path, State},
	http::StatusCode,
	Json,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::{errors::ServerError, AppState, ServerResult};
use barreleye_common::models::{
	optional_set, Account, AccountActiveModel, BasicModel,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Payload {
	name: Option<String>,
}

pub async fn handler(
	State(app): State<Arc<AppState>>,
	Path(account_id): Path<String>,
	Json(payload): Json<Payload>,
) -> ServerResult<StatusCode> {
	let update_data = AccountActiveModel {
		name: optional_set(payload.name),
		..Default::default()
	};

	if Account::update_by_id(&app.db, &account_id, update_data).await? {
		Ok(StatusCode::NO_CONTENT)
	} else {
		Err(ServerError::NotFound)
	}
}
