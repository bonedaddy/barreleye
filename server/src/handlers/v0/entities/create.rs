use axum::{extract::State, Json};
use serde::Deserialize;
use std::sync::Arc;

use crate::{errors::ServerError, App, ServerResult};
use barreleye_common::models::{BasicModel, Entity};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Payload {
	name: String,
	description: String,
}

pub async fn handler(
	State(app): State<Arc<App>>,
	Json(payload): Json<Payload>,
) -> ServerResult<Json<Entity>> {
	// check for duplicate name
	if Entity::get_by_name(&app.db, &payload.name, None).await?.is_some() {
		return Err(ServerError::Duplicate { field: "name".to_string(), value: payload.name });
	}

	// create new
	let entity_id =
		Entity::create(&app.db, Entity::new_model(&payload.name, &payload.description)).await?;

	// return newly created
	Ok(Entity::get(&app.db, entity_id).await?.unwrap().into())
}
