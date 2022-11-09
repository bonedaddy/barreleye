use axum::{
	extract::{Path, State},
	Json,
};
use std::sync::Arc;

use crate::{errors::ServerError, AppState, ServerResult};
use barreleye_common::models::{BasicModel, Label};

pub async fn handler(
	State(app): State<Arc<AppState>>,
	Path(label_id): Path<String>,
) -> ServerResult<Json<Label>> {
	Label::get_by_id(&app.db, &label_id)
		.await?
		.map(|v| v.into())
		.ok_or(ServerError::NotFound)
}
