use axum::{
	extract::{Path, State},
	Json,
};
use std::sync::Arc;

use crate::{errors::ServerError, App, ServerResult};
use barreleye_common::models::{BasicModel, Label};

pub async fn handler(
	State(app): State<Arc<App>>,
	Path(label_id): Path<String>,
) -> ServerResult<Json<Label>> {
	match Label::get_by_id(&app.db, &label_id).await? {
		Some(label) if !label.is_deleted => Ok(label.into()),
		_ => Err(ServerError::NotFound),
	}
}
