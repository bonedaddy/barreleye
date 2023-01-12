use axum::{
	extract::{Path, State},
	Json,
};
use std::sync::Arc;

use crate::{errors::ServerError, App, ServerResult};
use barreleye_common::models::{BasicModel, LabeledAddress};

pub async fn handler(
	State(app): State<Arc<App>>,
	Path(label_address_id): Path<String>,
) -> ServerResult<Json<LabeledAddress>> {
	match LabeledAddress::get_by_id(&app.db, &label_address_id).await? {
		Some(labeled_address) if !labeled_address.is_deleted => Ok(labeled_address.into()),
		_ => Err(ServerError::NotFound),
	}
}
