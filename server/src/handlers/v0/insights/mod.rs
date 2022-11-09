use axum::{routing::get, Router};
use std::sync::Arc;

use crate::{server::wrap_router, AppState};

mod get;

pub fn get_routes(app_state: Arc<AppState>) -> Router<Arc<AppState>> {
	wrap_router(Router::with_state(app_state).route("/", get(get::handler)))
}
