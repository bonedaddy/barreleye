use axum::{
	extract::{Path, State},
	Json,
};
use std::sync::Arc;

use crate::{errors::ServerError, App, ServerResult};
use barreleye_common::models::{ApiKey, BasicModel};

pub async fn handler(
	State(app): State<Arc<App>>,
	Path(api_key_id): Path<String>,
) -> ServerResult<Json<ApiKey>> {
	ApiKey::get_by_id(&app.db, &api_key_id)
		.await?
		.map(|ak| ak.format().into())
		.ok_or(ServerError::NotFound)
}
