use axum::{
	extract::{Path, State},
	http::StatusCode,
};
use std::sync::Arc;

use crate::{errors::ServerError, AppState, ServerResult};
use barreleye_common::models::{BasicModel, Config, Network};

pub async fn handler(
	State(app): State<Arc<AppState>>,
	Path(network_id): Path<String>,
) -> ServerResult<StatusCode> {
	if Network::delete_by_id(&app.db, &network_id).await? {
		Config::delete_all_by_keyword(&app.db, &format!("n{network_id}")).await?;
		Ok(StatusCode::NO_CONTENT)
	} else {
		Err(ServerError::NotFound)
	}
}
