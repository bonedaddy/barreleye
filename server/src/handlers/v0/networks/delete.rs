use axum::{
	extract::{Path, State},
	http::StatusCode,
};
use std::sync::Arc;

use crate::{errors::ServerError, ServerResult, ServerState};
use barreleye_common::models::{BasicModel, Network};

pub async fn handler(
	State(app): State<Arc<ServerState>>,
	Path(network_id): Path<String>,
) -> ServerResult<StatusCode> {
	if Network::delete_by_id(&app.db, &network_id).await? {
		Ok(StatusCode::NO_CONTENT)
	} else {
		Err(ServerError::NotFound)
	}
}
