use axum::{extract::State, Json};
use serde::Deserialize;
use std::sync::Arc;

use crate::{errors::ServerError, utils::extract_primary_ids, App, ServerResult};
use barreleye_common::models::{BasicModel, Entity, EntityTags, Tag};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Payload {
	name: Option<String>,
	description: String,
	tags: Option<Vec<String>>,
}

pub async fn handler(
	State(app): State<Arc<App>>,
	Json(payload): Json<Payload>,
) -> ServerResult<Json<Entity>> {
	// get a list of tag primary ids, while checking for invalid payload ids
	let tag_ids = {
		let mut ret = vec![];

		if let Some(tags) = payload.tags {
			ret = extract_primary_ids(
				"tags",
				tags.clone(),
				Tag::get_all_by_ids(app.db(), tags)
					.await?
					.into_iter()
					.map(|t| (t.id, t.tag_id))
					.collect(),
			)?;
		}

		ret
	};

	// check for duplicate name
	if let Some(name) = payload.name.clone() {
		if Entity::get_by_name(app.db(), &name, None).await?.is_some() {
			return Err(ServerError::Duplicate { field: "name".to_string(), value: name });
		}
	}

	// create new
	let entity_id =
		Entity::create(app.db(), Entity::new_model(payload.name, &payload.description)).await?;

	// upsert entity/tag mappings
	if !tag_ids.is_empty() {
		EntityTags::create_many(
			app.db(),
			tag_ids.into_iter().map(|tag_id| EntityTags::new_model(entity_id, tag_id)).collect(),
		)
		.await?;
	}

	// return newly created
	Ok(Entity::get(app.db(), entity_id).await?.unwrap().into())
}
