use axum::Router;
use std::sync::Arc;

use crate::{server::wrap_router, AppState};

mod accounts;
mod addresses;
mod heartbeat;
mod insights;
mod keys;
mod labels;
mod networks;

pub fn get_routes(app_state: Arc<AppState>) -> Router<Arc<AppState>> {
	wrap_router(
		Router::with_state(app_state.clone())
			.nest("/heartbeat", heartbeat::get_routes(app_state.clone()))
			.nest("/accounts", accounts::get_routes(app_state.clone()))
			.nest("/keys", keys::get_routes(app_state.clone()))
			.nest("/networks", networks::get_routes(app_state.clone()))
			.nest("/labels", labels::get_routes(app_state.clone()))
			.nest("/addresses", addresses::get_routes(app_state.clone()))
			.nest("/insights", insights::get_routes(app_state)),
	)
}
