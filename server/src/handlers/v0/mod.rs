use axum::Router;
use std::sync::Arc;

use crate::App;

mod addresses;
mod heartbeat;
mod info;
mod keys;
mod labels;
mod networks;
mod stats;
mod upstream;

pub fn get_routes() -> Router<Arc<App>> {
	Router::new()
		.nest("/heartbeat", heartbeat::get_routes())
		.nest("/stats", stats::get_routes())
		.nest("/keys", keys::get_routes())
		.nest("/networks", networks::get_routes())
		.nest("/labels", labels::get_routes())
		.nest("/addresses", addresses::get_routes())
		.nest("/info", info::get_routes())
		.nest("/upstream", upstream::get_routes())
}
