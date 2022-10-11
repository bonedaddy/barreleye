use axum::Router;
use std::sync::Arc;

use crate::ServerState;

mod v0;

pub fn get_routes(shared_state: Arc<ServerState>) -> Router {
	Router::new().nest("/v0", v0::get_routes(shared_state))
}
