use axum::{
	extract::{Path, State},
	Json,
};
use std::sync::Arc;

use crate::{errors::ServerError, App, ServerResult};
use barreleye_common::models::{Label, SoftDeleteModel};

pub async fn handler(
	State(app): State<Arc<App>>,
	Path(label_id): Path<String>,
) -> ServerResult<Json<Label>> {
	Label::get_existing_by_id(&app.db, &label_id)
		.await?
		.map(|l| l.into())
		.ok_or(ServerError::NotFound)
}
