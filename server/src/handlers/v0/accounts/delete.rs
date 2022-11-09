use axum::{
	extract::{Path, State},
	http::StatusCode,
};
use std::sync::Arc;

use crate::{errors::ServerError, ServerResult, ServerState};
use barreleye_common::models::{Account, ApiKey, BasicModel};

pub async fn handler(
	State(app): State<Arc<ServerState>>,
	Path(account_id): Path<String>,
) -> ServerResult<StatusCode> {
	let account = ApiKey::get_by_id(&app.db, &account_id)
		.await?
		.ok_or(ServerError::NotFound)?;

	// dont delete if has active keys
	let api_keys =
		ApiKey::get_all_by_account_id(&app.db, account.account_id).await?;
	if !api_keys.is_empty() {
		return Err(ServerError::BadRequest {
			reason: format!(
				"cannot delete with active api keys ({})",
				api_keys[..3]
					.iter()
					.map(|ak| format!("`{}`", ak.format().key))
					.collect::<Vec<String>>()
					.join(", ")
			),
		});
	}

	// delete
	if Account::delete_by_id(&app.db, &account_id).await? {
		Ok(StatusCode::NO_CONTENT)
	} else {
		Err(ServerError::NotFound)
	}
}
