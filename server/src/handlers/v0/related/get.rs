use axum::Json;
use serde::Serialize;

use crate::ServerResult;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Response {
	related: bool,
}

pub async fn handler() -> ServerResult<Json<Response>> {
	let response = Response { related: true };
	Ok(response.into())
}
