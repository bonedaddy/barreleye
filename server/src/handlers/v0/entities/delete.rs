use axum::{
	extract::{Path, State},
	http::StatusCode,
};
use std::sync::Arc;

use crate::{errors::ServerError, App, ServerResult};
use barreleye_common::models::{
	set, Address, AddressActiveModel, BasicModel, Entity, EntityActiveModel, SoftDeleteModel,
};

pub async fn handler(
	State(app): State<Arc<App>>,
	Path(entity_id): Path<String>,
) -> ServerResult<StatusCode> {
	if let Some(entity) = Entity::get_existing_by_id(&app.db, &entity_id).await? {
		// delete all associated addresses
		Address::update_by_entity_id(
			&app.db,
			entity.entity_id,
			AddressActiveModel { is_deleted: set(true), ..Default::default() },
		)
		.await?;

		// delete entity
		Entity::update_by_id(
			&app.db,
			&entity_id,
			EntityActiveModel { is_deleted: set(true), ..Default::default() },
		)
		.await?;

		Ok(StatusCode::NO_CONTENT)
	} else {
		Err(ServerError::NotFound)
	}
}
