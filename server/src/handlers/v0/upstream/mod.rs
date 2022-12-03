use axum::{routing::get, Router};
use std::sync::Arc;

use crate::AppState;

mod get;

pub fn get_routes() -> Router<Arc<AppState>> {
	Router::new().route("/", get(get::handler))
}
