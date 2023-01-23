use axum::{
	extract::{Path, State},
	Json,
};
use axum_extra::extract::Query;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{
	errors::ServerError,
	utils::{get_addresses_data, get_tags_data},
	App, ServerResult,
};
use barreleye_common::{
	models::{Address, Entity, Network, SoftDeleteModel, Tag},
	Aux,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Payload {
	#[serde(default)]
	aux: Vec<Aux>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Response {
	entity: Entity,
	#[serde(skip_serializing_if = "Option::is_none")]
	tags: Option<Vec<Tag>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	addresses: Option<Vec<Address>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	networks: Option<Vec<Network>>,
}

pub async fn handler(
	State(app): State<Arc<App>>,
	Path(entity_id): Path<String>,
	Query(payload): Query<Payload>,
) -> ServerResult<Json<Response>> {
	if let Some(mut entity) = Entity::get_existing_by_id(app.db(), &entity_id).await? {
		let entity_ids = vec![entity.entity_id];
		let (tags_data, addresses_data) = tokio::join!(
			get_tags_data(app.clone(), payload.aux.clone(), entity_ids.clone()),
			get_addresses_data(app.clone(), payload.aux, entity_ids),
		);

		let tags = if let Some((_tags, map)) = tags_data? {
			entity.tags = map.get(&entity.entity_id).cloned().or(Some(vec![]));
			Some(_tags)
		} else {
			None
		};

		let (address_data, networks) = addresses_data?;
		let addresses = if let Some((_addresses, map)) = address_data {
			entity.addresses = map.get(&entity.entity_id).cloned().or(Some(vec![]));
			Some(_addresses)
		} else {
			None
		};

		Ok(Response { entity, tags, addresses, networks }.into())
	} else {
		Err(ServerError::NotFound)
	}
}
