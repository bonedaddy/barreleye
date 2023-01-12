use axum::{
	extract::{Path, State},
	http::StatusCode,
};
use std::sync::Arc;

use crate::{errors::ServerError, App, ServerResult};
use barreleye_common::models::{
	set, BasicModel, LabeledAddress, LabeledAddressActiveModel, SoftDeleteModel,
};

pub async fn handler(
	State(app): State<Arc<App>>,
	Path(label_address_id): Path<String>,
) -> ServerResult<StatusCode> {
	if LabeledAddress::get_existing_by_id(&app.db, &label_address_id).await?.is_some() {
		LabeledAddress::update_by_id(
			&app.db,
			&label_address_id,
			LabeledAddressActiveModel { is_deleted: set(true), ..Default::default() },
		)
		.await?;

		Ok(StatusCode::NO_CONTENT)
	} else {
		Err(ServerError::NotFound)
	}
}
