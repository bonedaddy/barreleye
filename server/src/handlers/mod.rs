use axum::Router;
use std::sync::Arc;

use crate::AppState;

mod v0;

pub fn get_routes() -> Router<Arc<AppState>> {
	Router::new().nest("/v0", v0::get_routes())
}
