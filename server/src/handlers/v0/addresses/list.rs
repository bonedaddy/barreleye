use axum::{extract::State, Json};
use axum_extra::extract::Query;
use sea_orm::ColumnTrait;
use serde::Deserialize;
use std::sync::Arc;

use crate::{errors::ServerError, App, ServerResult};
use barreleye_common::models::{
	address::Column::{EntityId as AddressEntityId, IsDeleted as AddressIsDeleted},
	Address, BasicModel, Entity, SoftDeleteModel,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Payload {
	entity: Option<String>,
	offset: Option<u64>,
	limit: Option<u64>,
}

pub async fn handler(
	State(app): State<Arc<App>>,
	Query(payload): Query<Payload>,
) -> ServerResult<Json<Vec<Address>>> {
	let mut conditions = vec![AddressIsDeleted.eq(false)];

	if let Some(entity_id) = payload.entity {
		if let Some(entity) = Entity::get_existing_by_id(app.db(), &entity_id).await? {
			conditions.push(AddressEntityId.eq(entity.entity_id))
		} else {
			return Err(ServerError::InvalidParam {
				field: "entity".to_string(),
				value: entity_id,
			});
		}
	}

	Ok(Address::get_all_where(app.db(), conditions, payload.offset, payload.limit).await?.into())
}
