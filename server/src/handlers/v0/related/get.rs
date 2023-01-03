use axum::Json;
use serde::Serialize;

use crate::ServerResult;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Response {
	wip: bool,
}

pub async fn handler() -> ServerResult<Json<Response>> {
	let response = Response { wip: true };
	Ok(response.into())
}
