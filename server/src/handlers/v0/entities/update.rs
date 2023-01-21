use axum::{
	extract::{Path, State},
	http::StatusCode,
	Json,
};
use sea_orm::ActiveModelTrait;
use serde::Deserialize;
use std::sync::Arc;

use crate::{errors::ServerError, utils::extract_primary_ids, App, ServerResult};
use barreleye_common::models::{
	optional_set, BasicModel, Entity, EntityActiveModel, EntityTags, SoftDeleteModel, Tag,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Payload {
	name: Option<Option<String>>,
	tags: Option<Vec<String>>,
}

pub async fn handler(
	State(app): State<Arc<App>>,
	Path(entity_id): Path<String>,
	Json(payload): Json<Payload>,
) -> ServerResult<StatusCode> {
	if let Some(entity) = Entity::get_existing_by_id(app.db(), &entity_id).await? {
		// check for duplicate name
		if let Some(Some(name)) = payload.name.clone() {
			if let Some(other_entity) = Entity::get_by_name(app.db(), &name, None).await? {
				if other_entity.id != entity.id {
					return Err(ServerError::Duplicate { field: "name".to_string(), value: name });
				}
			}
		}

		// check for invalid tags
		let mut tag_ids = vec![];
		if let Some(tags) = payload.tags {
			tag_ids = extract_primary_ids(
				"tags",
				tags.clone(),
				Tag::get_all_by_ids(app.db(), tags)
					.await?
					.into_iter()
					.map(|t| (t.id, t.tag_id))
					.collect(),
			)?;
		}

		// update entity
		let update_data =
			EntityActiveModel { name: optional_set(payload.name), ..Default::default() };
		if update_data.is_changed() {
			Entity::update_by_id(app.db(), &entity_id, update_data).await?;
		}

		// upsert entity/tag mappings
		if !tag_ids.is_empty() {
			EntityTags::delete_not_included_tags(app.db(), entity.entity_id, tag_ids.clone())
				.await?;
			EntityTags::create_many(
				app.db(),
				tag_ids
					.iter()
					.map(|tag_id| EntityTags::new_model(entity.entity_id, *tag_id))
					.collect(),
			)
			.await?;
		}

		Ok(StatusCode::NO_CONTENT)
	} else {
		Err(ServerError::NotFound)
	}
}