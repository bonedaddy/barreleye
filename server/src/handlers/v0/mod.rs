use axum::Router;
use std::sync::Arc;

use crate::ServerState;

mod insights;

pub fn get_routes(shared_state: Arc<ServerState>) -> Router {
	Router::new().nest("/insights", insights::get_routes(shared_state))
}
