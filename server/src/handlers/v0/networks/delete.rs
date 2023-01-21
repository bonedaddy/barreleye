use axum::{
	extract::{Path, State},
	http::StatusCode,
};
use std::sync::Arc;

use crate::{errors::ServerError, App, ServerResult};
use barreleye_common::models::{BasicModel, Config, ConfigKey, Network};

pub async fn handler(
	State(app): State<Arc<App>>,
	Path(network_id): Path<String>,
) -> ServerResult<StatusCode> {
	if let Some(network) = Network::get_by_id(app.db(), &network_id).await? {
		Network::delete(app.db(), network.network_id).await?;
		Config::delete_all_by_keywords(app.db(), vec![format!("n{network_id}")]).await?;

		// update config
		if network.is_active {
			Config::set::<_, u8>(app.db(), ConfigKey::NetworksUpdated, 1).await?;
		}

		// update app's networks
		let mut networks = app.networks.write().await;
		*networks = app.get_networks().await?;

		Ok(StatusCode::NO_CONTENT)
	} else {
		Err(ServerError::NotFound)
	}
}
