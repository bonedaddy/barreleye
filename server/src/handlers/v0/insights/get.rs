use axum::{
	extract::{Query, State},
	Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{ServerResult, ServerState};
use barreleye_common::models::{
	sanctioned_address::Status as SanctionedAddressStatus, SanctionedAddress,
};

#[derive(Deserialize)]
pub struct Payload {
	address: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseOverview {
	net_worth: u64,
	net_worth_currency: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseCompliance {
	status: SanctionedAddressStatus,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Response {
	address: String,
	overview: ResponseOverview,
	compliance: ResponseCompliance,
}

pub async fn handler(
	State(app): State<Arc<ServerState>>,
	Query(payload): Query<Payload>,
) -> ServerResult<Json<Response>> {
	let mut response = Response {
		address: payload.address.clone(),
		overview: ResponseOverview {
			net_worth: 0,
			net_worth_currency: "USD".to_string(),
		},
		compliance: ResponseCompliance {
			status: SanctionedAddressStatus::NoIssuesFound,
		},
	};

	if SanctionedAddress::get_by_address(&app.db, &payload.address)
		.await?
		.is_some()
	{
		response.compliance.status = SanctionedAddressStatus::Sanctioned;
	}

	Ok(response.into())
}
