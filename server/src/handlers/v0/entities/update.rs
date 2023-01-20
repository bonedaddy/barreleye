use axum::{
	extract::{Path, State},
	http::StatusCode,
	Json,
};
use sea_orm::ActiveModelTrait;
use serde::Deserialize;
use std::sync::Arc;

use crate::{errors::ServerError, App, ServerResult};
use barreleye_common::models::{
	optional_set, BasicModel, Entity, EntityActiveModel, SoftDeleteModel,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Payload {
	name: Option<String>,
}

pub async fn handler(
	State(app): State<Arc<App>>,
	Path(entity_id): Path<String>,
	Json(payload): Json<Payload>,
) -> ServerResult<StatusCode> {
	if let Some(entity) = Entity::get_existing_by_id(&app.db, &entity_id).await? {
		// check for duplicate name
		if let Some(name) = payload.name.clone() {
			if let Some(other_entity) = Entity::get_by_name(&app.db, &name, None).await? {
				if other_entity.id != entity.id {
					return Err(ServerError::Duplicate { field: "name".to_string(), value: name });
				}
			}
		}

		// update
		let update_data =
			EntityActiveModel { name: optional_set(payload.name), ..Default::default() };
		if update_data.is_changed() {
			Entity::update_by_id(&app.db, &entity_id, update_data).await?;
		}

		Ok(StatusCode::NO_CONTENT)
	} else {
		Err(ServerError::NotFound)
	}
}
