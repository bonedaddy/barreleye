use axum::Router;
use std::sync::Arc;

use crate::App;

mod v0;

pub fn get_routes() -> Router<Arc<App>> {
	Router::new().nest("/v0", v0::get_routes())
}
