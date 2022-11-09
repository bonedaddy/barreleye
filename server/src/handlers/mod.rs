use axum::Router;
use std::sync::Arc;

use crate::{server::wrap_router, AppState};

mod v0;

pub fn get_routes(app_state: Arc<AppState>) -> Router<Arc<AppState>> {
	wrap_router(
		Router::with_state(app_state.clone())
			.nest("/v0", v0::get_routes(app_state)),
	)
}
