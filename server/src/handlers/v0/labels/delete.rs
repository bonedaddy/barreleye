use axum::{
	extract::{Path, State},
	http::StatusCode,
};
use std::sync::Arc;

use crate::{errors::ServerError, ServerResult, ServerState};
use barreleye_common::models::{BasicModel, Label, LabeledAddress};

pub async fn handler(
	State(app): State<Arc<ServerState>>,
	Path(label_id): Path<String>,
) -> ServerResult<StatusCode> {
	let label = Label::get_by_id(&app.db, &label_id)
		.await?
		.ok_or(ServerError::NotFound)?;

	// dont delete if hardcoded
	if label.is_hardcoded {
		return Err(ServerError::BadRequest {
			reason: "cannot delete a hardcoded label".to_string(),
		});
	}

	// dont delete if applied
	let labeled_addresses =
		LabeledAddress::get_all_by_label_ids(&app.db, vec![label.label_id])
			.await?;
	if !labeled_addresses.is_empty() {
		return Err(ServerError::BadRequest {
			reason: format!(
				"cannot delete applied label ({})",
				labeled_addresses[..3]
					.iter()
					.map(|la| format!("`{}`", la.id))
					.collect::<Vec<String>>()
					.join(", ")
			),
		});
	}

	// delete
	if Label::delete_by_id(&app.db, &label_id).await? {
		Ok(StatusCode::NO_CONTENT)
	} else {
		Err(ServerError::NotFound)
	}
}
