use axum::{
	extract::{Path, State},
	Json,
};
use std::sync::Arc;

use crate::{errors::ServerError, App, ServerResult};
use barreleye_common::models::{BasicModel, Tag};

pub async fn handler(
	State(app): State<Arc<App>>,
	Path(tag_id): Path<String>,
) -> ServerResult<Json<Tag>> {
	Tag::get_by_id(&app.db, &tag_id).await?.map(|a| a.into()).ok_or(ServerError::NotFound)
}
