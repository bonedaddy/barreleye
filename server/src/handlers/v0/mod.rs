use axum::Router;
use std::sync::Arc;

use crate::AppState;

mod accounts;
mod addresses;
mod heartbeat;
mod insights;
mod keys;
mod labels;
mod networks;
mod stats;

pub fn get_routes() -> Router<Arc<AppState>> {
	Router::new()
		.nest("/heartbeat", heartbeat::get_routes())
		.nest("/stats", stats::get_routes())
		.nest("/accounts", accounts::get_routes())
		.nest("/keys", keys::get_routes())
		.nest("/networks", networks::get_routes())
		.nest("/labels", labels::get_routes())
		.nest("/addresses", addresses::get_routes())
		.nest("/insights", insights::get_routes())
}
