use axum::{extract::State, Json};
use sea_orm::ColumnTrait;
use serde::Deserialize;
use std::sync::Arc;

use crate::{errors::ServerError, App, ServerResult};
use barreleye_common::models::{
	labeled_address::Column::{
		IsDeleted as LabeledAddressIsDeleted, LabelId as LabeledAddressLabelId,
	},
	BasicModel, Label, LabeledAddress,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Payload {
	label: Option<String>,
	offset: Option<u64>,
	limit: Option<u64>,
}

pub async fn handler(
	State(app): State<Arc<App>>,
	Json(payload): Json<Option<Payload>>,
) -> ServerResult<Json<Vec<LabeledAddress>>> {
	let mut conditions = vec![LabeledAddressIsDeleted.eq(false)];

	let mut offset = None;
	let mut limit = None;

	if let Some(payload) = payload {
		if let Some(label_id) = payload.label {
			match Label::get_by_id(&app.db, &label_id).await? {
				Some(label) if !label.is_deleted => {
					conditions.push(LabeledAddressLabelId.eq(label.label_id))
				}
				_ => {
					return Err(ServerError::InvalidParam {
						field: "label".to_string(),
						value: label_id,
					});
				}
			}
		}

		offset = payload.offset;
		limit = payload.limit;
	}

	Ok(LabeledAddress::get_all_where(&app.db, conditions, offset, limit).await?.into())
}
