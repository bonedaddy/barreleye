use axum::{routing::get, Router};
use std::sync::Arc;

use barreleye_common::AppState;

pub mod insights;

pub fn get_routes(shared_state: Arc<AppState>) -> Router<Arc<AppState>> {
	Router::with_state(shared_state)
		.route("/insights", get(insights::get::handler))
}
