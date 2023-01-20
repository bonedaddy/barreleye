use axum::{
	extract::{Path, State},
	Json,
};
use std::sync::Arc;

use crate::{errors::ServerError, App, ServerResult};
use barreleye_common::models::{Address, SoftDeleteModel};

pub async fn handler(
	State(app): State<Arc<App>>,
	Path(address_id): Path<String>,
) -> ServerResult<Json<Address>> {
	Address::get_existing_by_id(&app.db, &address_id)
		.await?
		.map(|a| a.into())
		.ok_or(ServerError::NotFound)
}
