use axum::{extract::State, Json};
use sea_orm::ColumnTrait;
use serde::Deserialize;
use std::sync::Arc;

use crate::{errors::ServerError, AppState, ServerResult};
use barreleye_common::models::{
	account::Column::AccountId as AccountAccountId, Account, ApiKey, BasicModel,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Payload {
	account: Option<String>,
	offset: Option<u64>,
	limit: Option<u64>,
}

pub async fn handler(
	State(app): State<Arc<AppState>>,
	Json(payload): Json<Option<Payload>>,
) -> ServerResult<Json<Vec<ApiKey>>> {
	let mut conditions = vec![];

	let mut offset = None;
	let mut limit = None;

	if let Some(payload) = payload {
		if let Some(account_id) = payload.account {
			match Account::get_by_id(&app.db, &account_id).await? {
				Some(account) => {
					conditions.push(AccountAccountId.eq(account.account_id))
				}
				_ => {
					return Err(ServerError::InvalidParam {
						field: "account".to_string(),
						value: account_id,
					});
				}
			}
		}

		offset = payload.offset;
		limit = payload.limit;
	}

	Ok(ApiKey::get_all_where(&app.db, conditions, offset, limit)
		.await?
		.iter()
		.map(|ak| ak.format())
		.collect::<Vec<ApiKey>>()
		.into())
}
