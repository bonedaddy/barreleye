use axum::{
	routing::{delete, get, post, put},
	Router,
};
use std::sync::Arc;

use crate::{server::wrap_router, AppState};

mod create;
mod delete;
mod get;
mod list;
mod update;

pub fn get_routes(app_state: Arc<AppState>) -> Router<Arc<AppState>> {
	wrap_router(
		Router::with_state(app_state)
			.route("/", post(create::handler))
			.route("/", get(list::handler))
			.route("/:id", get(get::handler))
			.route("/:id", put(update::handler))
			.route("/:id", delete(delete::handler)),
	)
}
