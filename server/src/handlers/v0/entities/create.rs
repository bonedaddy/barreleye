use axum::{extract::State, Json};
use serde::Deserialize;
use std::sync::Arc;

use crate::{errors::ServerError, utils, App, ServerResult};
use barreleye_common::models::{BasicModel, Entity, EntityTagMap, Tag};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Payload {
	name: String,
	description: String,
	tags: Vec<String>,
}

pub async fn handler(
	State(app): State<Arc<App>>,
	Json(payload): Json<Payload>,
) -> ServerResult<Json<Entity>> {
	// get a list of tag primary ids, while checking for invalid payload ids
	let tag_ids = utils::extract_primary_ids(
		"tags",
		payload.tags.clone(),
		Tag::get_all_by_ids(&app.db, payload.tags)
			.await?
			.into_iter()
			.map(|t| (t.id, t.tag_id))
			.collect(),
	)?;

	// check for duplicate name
	if Entity::get_by_name(&app.db, &payload.name, None).await?.is_some() {
		return Err(ServerError::Duplicate { field: "name".to_string(), value: payload.name });
	}

	// create new
	let entity_id =
		Entity::create(&app.db, Entity::new_model(&payload.name, &payload.description)).await?;

	// upsert entity/tag mappings
	if !tag_ids.is_empty() {
		EntityTagMap::create_many(
			&app.db,
			tag_ids.into_iter().map(|tag_id| EntityTagMap::new_model(entity_id, tag_id)).collect(),
		)
		.await?;
	}

	// return newly created
	Ok(Entity::get(&app.db, entity_id).await?.unwrap().into())
}
