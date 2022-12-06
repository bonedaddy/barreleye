use axum::{
	extract::{Path, State},
	http::StatusCode,
	Json,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::{errors::ServerError, AppState, ServerResult};
use barreleye_common::models::{optional_set, BasicModel, Label, LabelActiveModel};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Payload {
	name: Option<String>,
	is_enabled: Option<bool>,
	is_tracked: Option<bool>,
}

pub async fn handler(
	State(app): State<Arc<AppState>>,
	Path(label_id): Path<String>,
	Json(payload): Json<Payload>,
) -> ServerResult<StatusCode> {
	let label = Label::get_by_id(&app.db, &label_id).await?.ok_or(ServerError::NotFound)?;

	// check for duplicate name
	if let Some(name) = payload.name.clone() {
		if label_id != label.id && label.name.trim().to_lowercase() == name.trim().to_lowercase() {
			return Err(ServerError::Duplicate { field: "name".to_string(), value: name });
		}
	}

	let update_data = LabelActiveModel {
		name: optional_set(payload.name),
		is_enabled: optional_set(payload.is_enabled),
		is_tracked: optional_set(payload.is_tracked),
		..Default::default()
	};

	if Label::update_by_id(&app.db, &label_id, update_data).await? {
		Ok(StatusCode::NO_CONTENT)
	} else {
		Err(ServerError::NotFound)
	}
}
