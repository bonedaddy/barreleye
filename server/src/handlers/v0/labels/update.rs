use axum::{
	extract::{Path, State},
	http::StatusCode,
	Json,
};
use sea_orm::entity::ActiveValue;
use serde::Deserialize;
use std::sync::Arc;

use crate::{errors::ServerError, ServerResult, ServerState};
use barreleye_common::models::{BasicModel, Label, LabelActiveModel};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Payload {
	name: Option<String>,
	is_enabled: Option<bool>,
	is_tracked: Option<bool>,
}

pub async fn handler(
	State(app): State<Arc<ServerState>>,
	Path(label_id): Path<String>,
	Json(payload): Json<Payload>,
) -> ServerResult<StatusCode> {
	let update_data = LabelActiveModel {
		name: match payload.name {
			Some(name) => ActiveValue::set(name),
			_ => ActiveValue::not_set(),
		},
		is_enabled: match payload.is_enabled {
			Some(is_enabled) => ActiveValue::set(is_enabled),
			_ => ActiveValue::not_set(),
		},
		is_tracked: match payload.is_tracked {
			Some(is_tracked) => ActiveValue::set(is_tracked),
			_ => ActiveValue::not_set(),
		},
		..Default::default()
	};

	if Label::update_by_id(&app.db, &label_id, update_data).await? {
		Ok(StatusCode::NO_CONTENT)
	} else {
		Err(ServerError::NotFound)
	}
}
