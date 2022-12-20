use axum::{routing::get, Router};
use std::sync::Arc;

use crate::App;

mod get;

pub fn get_routes() -> Router<Arc<App>> {
	Router::new().route("/", get(get::handler))
}
