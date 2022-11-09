use axum::{
	extract::{Path, State},
	Json,
};
use std::sync::Arc;

use crate::{errors::ServerError, AppState, ServerResult};
use barreleye_common::models::{Account, BasicModel};

pub async fn handler(
	State(app): State<Arc<AppState>>,
	Path(account_id): Path<String>,
) -> ServerResult<Json<Account>> {
	Account::get_by_id(&app.db, &account_id)
		.await?
		.map(|v| v.into())
		.ok_or(ServerError::NotFound)
}
