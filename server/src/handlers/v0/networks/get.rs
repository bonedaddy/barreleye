use axum::{
	extract::{Path, State},
	Json,
};
use std::sync::Arc;

use crate::{errors::ServerError, AppState, ServerResult};
use barreleye_common::models::{BasicModel, Network};

pub async fn handler(
	State(app): State<Arc<AppState>>,
	Path(network_id): Path<String>,
) -> ServerResult<Json<Network>> {
	Network::get_by_id(&app.db, &network_id)
		.await?
		.map(|v| v.into())
		.ok_or(ServerError::NotFound)
}
