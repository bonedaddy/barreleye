use axum::{
	extract::{Extension, State},
	Json,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::{errors::ServerError, AppState, ServerResult};
use barreleye_common::models::{Account, ApiKey, BasicModel};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Payload {
	account: Option<String>,
	is_admin: bool,
}

pub async fn handler(
	Extension(mut account): Extension<Account>,
	State(app): State<Arc<AppState>>,
	Json(payload): Json<Payload>,
) -> ServerResult<Json<ApiKey>> {
	// get either passed account, or self
	if let Some(account_id) = payload.account {
		account = Account::get_by_id(&app.db, &account_id).await?.ok_or(
			ServerError::InvalidParam {
				field: "account".to_string(),
				value: account_id,
			},
		)?;
	}

	// create new
	let api_key_id = ApiKey::create(
		&app.db,
		ApiKey::new_model(account.account_id, payload.is_admin),
	)
	.await?;

	// return newly created
	Ok(ApiKey::get(&app.db, api_key_id).await?.unwrap().format().into())
}
