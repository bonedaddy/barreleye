use axum::Router;
use std::sync::Arc;

use crate::{wrap_router, ServerState};

mod v0;

pub fn get_routes(shared_state: Arc<ServerState>) -> Router<Arc<ServerState>> {
	wrap_router(
		Router::with_state(shared_state.clone())
			.nest("/v0", v0::get_routes(shared_state)),
	)
}
