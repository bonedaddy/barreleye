use axum::Router;
use std::sync::Arc;

use crate::{server::wrap_router, ServerState};

mod addresses;
mod heartbeat;
mod insights;
mod keys;
mod labels;

pub fn get_routes(shared_state: Arc<ServerState>) -> Router<Arc<ServerState>> {
	wrap_router(
		Router::with_state(shared_state.clone())
			.nest("/heartbeat", heartbeat::get_routes(shared_state.clone()))
			.nest("/keys", keys::get_routes(shared_state.clone()))
			.nest("/labels", labels::get_routes(shared_state.clone()))
			.nest("/addresses", addresses::get_routes(shared_state.clone()))
			.nest("/insights", insights::get_routes(shared_state)),
	)
}
