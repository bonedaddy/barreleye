use axum::Router;
use std::sync::Arc;

use crate::{wrap_router, ServerState};

mod heartbeat;
mod insights;

pub fn get_routes(shared_state: Arc<ServerState>) -> Router<Arc<ServerState>> {
	wrap_router(
		Router::with_state(shared_state.clone())
			.nest("/heartbeat", heartbeat::get_routes(shared_state.clone()))
			.nest("/insights", insights::get_routes(shared_state)),
	)
}
