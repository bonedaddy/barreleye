use axum::Router;
use std::sync::Arc;

use barreleye_common::AppState;

pub mod v0;

pub fn get_routes(shared_state: Arc<AppState>) -> Router {
	Router::new().nest("/v0", v0::get_routes(shared_state))
}