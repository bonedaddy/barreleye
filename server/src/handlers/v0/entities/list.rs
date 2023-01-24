use axum::{
	extract::{Query, State},
	Json,
};
use sea_orm::ColumnTrait;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};

use crate::{
	handlers::v0::entities::{get_addresses_data, get_tags_data},
	ServerResult,
};
use barreleye_common::{
	models::{
		entity::Column::IsDeleted as EntityIsDeleted, Address, BasicModel, Entity, Network,
		PrimaryId, Tag,
	},
	App,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Payload {
	offset: Option<u64>,
	limit: Option<u64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Response {
	entities: Vec<Entity>,
	tags: Vec<Tag>,
	addresses: Vec<Address>,
	networks: Vec<Network>,
}

pub async fn handler(
	State(app): State<Arc<App>>,
	Query(payload): Query<Payload>,
) -> ServerResult<Json<Response>> {
	let mut entities = Entity::get_all_where(
		app.db(),
		vec![EntityIsDeleted.eq(false)],
		payload.offset,
		payload.limit,
	)
	.await?
	.into_iter()
	.map(|e| (e.entity_id, e))
	.collect::<HashMap<PrimaryId, Entity>>();

	let entity_ids = entities.clone().into_keys().collect::<Vec<PrimaryId>>();
	let (tags_data, addresses_data) = tokio::join!(
		get_tags_data(app.clone(), entity_ids.clone()),
		get_addresses_data(app.clone(), entity_ids),
	);

	let (tags, tags_map) = tags_data?;
	for (entity_id, entity) in entities.iter_mut() {
		entity.tags = tags_map.get(entity_id).cloned().or(Some(vec![]));
	}

	let (addresses, addresses_map, networks) = addresses_data?;
	for (entity_id, entity) in entities.iter_mut() {
		entity.addresses = addresses_map.get(entity_id).cloned().or(Some(vec![]));
	}

	Ok(Response { entities: entities.into_values().collect(), tags, addresses, networks }.into())
}
