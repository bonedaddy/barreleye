use axum::{
	extract::{Path, State},
	http::StatusCode,
};
use std::sync::Arc;

use crate::{errors::ServerError, AppState, ServerResult};
use barreleye_common::models::{BasicModel, Config, ConfigKey, Network};

pub async fn handler(
	State(app): State<Arc<AppState>>,
	Path(network_id): Path<String>,
) -> ServerResult<StatusCode> {
	if let Some(network) = Network::get_by_id(&app.db, &network_id).await? {
		Network::delete(&app.db, network.network_id).await?;
		Config::delete_all_by_keyword(&app.db, &format!("n{network_id}")).await?;

		// update config
		if network.is_active {
			Config::set::<u8>(&app.db, ConfigKey::NetworksUpdated, 1).await?;
		}

		Ok(StatusCode::NO_CONTENT)
	} else {
		Err(ServerError::NotFound)
	}
}
