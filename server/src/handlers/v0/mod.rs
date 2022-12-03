use axum::Router;
use std::sync::Arc;

use crate::AppState;

mod addresses;
mod assets;
mod heartbeat;
mod keys;
mod labels;
mod networks;
mod related;
mod stats;
mod upstream;

pub fn get_routes() -> Router<Arc<AppState>> {
	Router::new()
		.nest("/heartbeat", heartbeat::get_routes())
		.nest("/stats", stats::get_routes())
		.nest("/keys", keys::get_routes())
		.nest("/networks", networks::get_routes())
		.nest("/labels", labels::get_routes())
		.nest("/addresses", addresses::get_routes())
		.nest("/assets", assets::get_routes())
		.nest("/upstream", upstream::get_routes())
		.nest("/related", related::get_routes())
}
