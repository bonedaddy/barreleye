use axum::{
	extract::{Path, State},
	http::StatusCode,
};
use std::sync::Arc;

use crate::{errors::ServerError, App, ServerResult};
use barreleye_common::models::{set, BasicModel, LabeledAddress, LabeledAddressActiveModel};

pub async fn handler(
	State(app): State<Arc<App>>,
	Path(label_address_id): Path<String>,
) -> ServerResult<StatusCode> {
	if let Some(labeled_address) = LabeledAddress::get_by_id(&app.db, &label_address_id).await? {
		if !labeled_address.is_deleted {
			LabeledAddress::update_by_id(
				&app.db,
				&label_address_id,
				LabeledAddressActiveModel { is_deleted: set(true), ..Default::default() },
			)
			.await?;

			return Ok(StatusCode::NO_CONTENT);
		}
	}

	Err(ServerError::NotFound)
}
