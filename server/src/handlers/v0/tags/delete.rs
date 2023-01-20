use axum::{
	extract::{Path, State},
	http::StatusCode,
};
use std::sync::Arc;

use crate::{errors::ServerError, App, ServerResult};
use barreleye_common::models::{BasicModel, EntityTagMap, Tag};

pub async fn handler(
	State(app): State<Arc<App>>,
	Path(tag_id): Path<String>,
) -> ServerResult<StatusCode> {
	if let Some(tag) = Tag::get_by_id(&app.db, &tag_id).await? {
		// delete entity/tag mappings
		EntityTagMap::delete_all_by_tag_ids(&app.db, vec![tag.tag_id]).await?;

		// delete tag
		Tag::delete(&app.db, tag.tag_id).await?;

		Ok(StatusCode::NO_CONTENT)
	} else {
		Err(ServerError::NotFound)
	}
}
