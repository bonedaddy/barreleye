use axum::{
	extract::{Path, State},
	http::StatusCode,
};
use std::sync::Arc;

use crate::{errors::ServerError, ServerResult};
use barreleye_common::{
	models::{
		set, Address, AddressActiveModel, BasicModel, Config, ConfigKey, Network,
		NetworkActiveModel, SoftDeleteModel,
	},
	App,
};

pub async fn handler(
	State(app): State<Arc<App>>,
	Path(network_id): Path<String>,
) -> ServerResult<StatusCode> {
	if let Some(network) = Network::get_existing_by_id(app.db(), &network_id).await? {
		// soft-delete all associated addresses
		Address::update_by_network_id(
			app.db(),
			network.network_id,
			AddressActiveModel { is_deleted: set(true), ..Default::default() },
		)
		.await?;

		// soft-delete network
		Network::update_by_id(
			app.db(),
			&network_id,
			NetworkActiveModel { is_deleted: set(true), ..Default::default() },
		)
		.await?;

		// update config
		Config::set::<_, u8>(app.db(), ConfigKey::NetworksUpdated, 1).await?;

		// update app's networks
		let mut networks = app.networks.write().await;
		*networks = app.get_networks().await?;

		Ok(StatusCode::NO_CONTENT)
	} else {
		Err(ServerError::NotFound)
	}
}
