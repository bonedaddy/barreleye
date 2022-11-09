use axum::{extract::State, Json};
use serde::Deserialize;
use std::sync::Arc;

use crate::{AppState, ServerResult};
use barreleye_common::models::{Account, BasicModel};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Payload {
	name: String,
}

pub async fn handler(
	State(app): State<Arc<AppState>>,
	Json(payload): Json<Payload>,
) -> ServerResult<Json<Account>> {
	let account_id =
		Account::create(&app.db, Account::new_model(&payload.name)).await?;

	// return newly created
	Ok(Account::get(&app.db, account_id).await?.unwrap().into())
}
