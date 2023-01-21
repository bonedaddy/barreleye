use axum::{
	extract::{Path, State},
	Json,
};
use std::sync::Arc;

use crate::{errors::ServerError, App, ServerResult};
use barreleye_common::models::{Entity, SoftDeleteModel};

pub async fn handler(
	State(app): State<Arc<App>>,
	Path(entity_id): Path<String>,
) -> ServerResult<Json<Entity>> {
	Entity::get_existing_by_id(app.db(), &entity_id)
		.await?
		.map(|l| l.into())
		.ok_or(ServerError::NotFound)
}
