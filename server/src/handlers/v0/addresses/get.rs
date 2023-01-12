use axum::{
	extract::{Path, State},
	Json,
};
use std::sync::Arc;

use crate::{errors::ServerError, App, ServerResult};
use barreleye_common::models::{LabeledAddress, SoftDeleteModel};

pub async fn handler(
	State(app): State<Arc<App>>,
	Path(label_address_id): Path<String>,
) -> ServerResult<Json<LabeledAddress>> {
	LabeledAddress::get_existing_by_id(&app.db, &label_address_id)
		.await?
		.map(|a| a.into())
		.ok_or(ServerError::NotFound)
}
