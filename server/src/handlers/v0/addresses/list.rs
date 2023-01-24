use axum::{extract::State, Json};
use axum_extra::extract::Query;
use sea_orm::ColumnTrait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::ServerResult;
use barreleye_common::{
	models::{
		address::Column::IsDeleted as AddressIsDeleted, Address, BasicModel, Network, PrimaryId,
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
	addresses: Vec<Address>,
	networks: Vec<Network>,
}

pub async fn handler(
	State(app): State<Arc<App>>,
	Query(payload): Query<Payload>,
) -> ServerResult<Json<Response>> {
	let addresses = Address::get_all_where(
		app.db(),
		vec![AddressIsDeleted.eq(false)],
		payload.offset,
		payload.limit,
	)
	.await?;

	let network_ids = addresses.iter().map(|a| a.network_id).collect::<Vec<PrimaryId>>();
	let networks = Network::get_all_by_network_ids(app.db(), network_ids.into()).await?;

	Ok(Response { addresses, networks }.into())
}
