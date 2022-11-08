use axum::{
	extract::{Path, State},
	http::StatusCode,
	Json,
};
use sea_orm::entity::ActiveValue;
use serde::Deserialize;
use std::sync::Arc;

use crate::{errors::ServerError, ServerResult, ServerState};
use barreleye_common::models::{
	BasicModel, Label, LabeledAddress, LabeledAddressActiveModel,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Payload {
	label: Option<String>,
	address: Option<String>,
}

pub async fn handler(
	State(app): State<Arc<ServerState>>,
	Path(label_address_id): Path<String>,
	Json(payload): Json<Payload>,
) -> ServerResult<StatusCode> {
	let update_data = LabeledAddressActiveModel {
		label_id: match payload.label {
			Some(label) => {
				let label = Label::get_by_id(&app.db, &label).await?.ok_or(
					ServerError::InvalidParam {
						field: "label".to_string(),
						value: label,
					},
				)?;

				ActiveValue::set(label.label_id)
			}
			None => ActiveValue::not_set(),
		},
		address: match payload.address {
			Some(address) => ActiveValue::set(address),
			None => ActiveValue::not_set(),
		},
		..Default::default()
	};

	if LabeledAddress::update_by_id(&app.db, &label_address_id, update_data)
		.await?
	{
		Ok(StatusCode::NO_CONTENT)
	} else {
		Err(ServerError::NotFound)
	}
}
