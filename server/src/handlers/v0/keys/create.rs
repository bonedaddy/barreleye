use axum::{extract::State, Json};
use std::sync::Arc;

use crate::{App, ServerResult};
use barreleye_common::models::{ApiKey, BasicModel};

pub async fn handler(State(app): State<Arc<App>>) -> ServerResult<Json<ApiKey>> {
	// create new
	let api_key_id = ApiKey::create(&app.db, ApiKey::new_model()).await?;

	// return newly created
	Ok(ApiKey::get(&app.db, api_key_id).await?.unwrap().format().into())
}
