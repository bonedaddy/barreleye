use axum::{extract::State, Json};
use axum_extra::extract::Query;
use sea_orm::ColumnTrait;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};

use crate::{
	utils::{get_addresses_data, get_tags_data},
	App, ServerResult,
};
use barreleye_common::{
	models::{
		entity::Column::IsDeleted as EntityIsDeleted, Address, BasicModel, Entity, Network,
		PrimaryId, Tag,
	},
	Aux,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Payload {
	offset: Option<u64>,
	limit: Option<u64>,
	#[serde(default)]
	aux: Vec<Aux>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Response {
	entities: Vec<Entity>,
	#[serde(skip_serializing_if = "Option::is_none")]
	tags: Option<Vec<Tag>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	addresses: Option<Vec<Address>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	networks: Option<Vec<Network>>,
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
		get_tags_data(app.clone(), payload.aux.clone(), entity_ids.clone()),
		get_addresses_data(app.clone(), payload.aux, entity_ids),
	);

	let tags = if let Some((_tags, map)) = tags_data? {
		for (entity_id, entity) in entities.iter_mut() {
			entity.tags = map.get(entity_id).cloned().or(Some(vec![]));
		}

		Some(_tags)
	} else {
		None
	};

	let (address_data, networks) = addresses_data?;
	let addresses = if let Some((_addresses, map)) = address_data {
		for (entity_id, entity) in entities.iter_mut() {
			entity.addresses = map.get(entity_id).cloned().or(Some(vec![]));
		}

		Some(_addresses)
	} else {
		None
	};

	Ok(Response { entities: entities.into_values().collect(), tags, addresses, networks }.into())
}
