use axum::{
	extract::{Path, State},
	Json,
};
use std::sync::Arc;

use crate::{errors::ServerError, ServerResult, ServerState};
use barreleye_common::models::{BasicModel, Network};

pub async fn handler(
	State(app): State<Arc<ServerState>>,
	Path(network_id): Path<String>,
) -> ServerResult<Json<Network>> {
	Network::get_by_id(&app.db, &network_id)
		.await?
		.map(|v| v.into())
		.ok_or(ServerError::NotFound)
}
