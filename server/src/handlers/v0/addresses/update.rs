use axum::{
	extract::{Path, State},
	http::StatusCode,
	Json,
};
use sea_orm::entity::ActiveValue;
use serde::Deserialize;
use std::sync::Arc;

use crate::{errors::ServerError, App, ServerResult};
use barreleye_common::models::{
	optional_set, BasicModel, Label, LabeledAddress, LabeledAddressActiveModel,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Payload {
	label: Option<String>,
	address: Option<String>,
}

pub async fn handler(
	State(app): State<Arc<App>>,
	Path(label_address_id): Path<String>,
	Json(payload): Json<Payload>,
) -> ServerResult<StatusCode> {
	let update_data =
		LabeledAddressActiveModel {
			label_id: match payload.label {
				Some(label) => {
					let label = Label::get_by_id(&app.db, &label).await?.ok_or(
						ServerError::InvalidParam { field: "label".to_string(), value: label },
					)?;

					ActiveValue::set(label.label_id)
				}
				_ => ActiveValue::not_set(),
			},
			address: optional_set(payload.address),
			..Default::default()
		};

	if LabeledAddress::update_by_id(&app.db, &label_address_id, update_data).await? {
		Ok(StatusCode::NO_CONTENT)
	} else {
		Err(ServerError::NotFound)
	}
}
