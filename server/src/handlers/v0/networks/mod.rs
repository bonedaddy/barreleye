use axum::{
	routing::{delete, get, post, put},
	Router,
};
use std::sync::Arc;

use crate::{server::wrap_router, ServerState};

mod create;
mod delete;
mod get;
mod list;
mod update;

pub fn get_routes(shared_state: Arc<ServerState>) -> Router<Arc<ServerState>> {
	wrap_router(
		Router::with_state(shared_state)
			.route("/", post(create::handler))
			.route("/", get(list::handler))
			.route("/:id", get(get::handler))
			.route("/:id", put(update::handler))
			.route("/:id", delete(delete::handler)),
	)
}
