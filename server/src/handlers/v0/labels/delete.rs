use axum::{
	extract::{Path, State},
	http::StatusCode,
};
use std::sync::Arc;

use crate::{errors::ServerError, App, ServerResult};
use barreleye_common::models::{
	set, BasicModel, Label, LabelActiveModel, LabeledAddress, LabeledAddressActiveModel,
	SoftDeleteModel,
};

pub async fn handler(
	State(app): State<Arc<App>>,
	Path(label_id): Path<String>,
) -> ServerResult<StatusCode> {
	if let Some(label) = Label::get_existing_by_id(&app.db, &label_id).await? {
		// delete all associated addresses
		LabeledAddress::update_by_label_id(
			&app.db,
			label.label_id,
			LabeledAddressActiveModel { is_deleted: set(true), ..Default::default() },
		)
		.await?;

		// delete label
		Label::update_by_id(
			&app.db,
			&label_id,
			LabelActiveModel { is_deleted: set(true), ..Default::default() },
		)
		.await?;

		Ok(StatusCode::NO_CONTENT)
	} else {
		Err(ServerError::NotFound)
	}
}
