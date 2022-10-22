use axum::{
	http::StatusCode,
	response::{IntoResponse, Response},
	Json,
};
use derive_more::{Display, Error};
use eyre::{ErrReport, Report};
use sea_orm::DbErr;
use serde_json::json;

#[derive(Debug, Display, Error)]
pub enum ServerError {
	#[display(fmt = "unauthorized")]
	Unauthorized,

	#[display(fmt = "validation error on field: {field}")]
	Validation { field: String },

	#[display(fmt = "not found")]
	NotFound,

	#[display(fmt = "rekt")]
	Internal { error: Report },
}

impl IntoResponse for ServerError {
	fn into_response(self) -> Response {
		let http_code = match self {
			ServerError::Unauthorized => StatusCode::UNAUTHORIZED,
			ServerError::Validation { .. } => StatusCode::BAD_REQUEST,
			ServerError::NotFound => StatusCode::NOT_FOUND,
			ServerError::Internal { .. } => StatusCode::INTERNAL_SERVER_ERROR,
		};

		let body = Json(json!({
			"error": self.to_string(),
		}));

		(http_code, body).into_response()
	}
}

impl From<DbErr> for ServerError {
	fn from(e: DbErr) -> ServerError {
		ServerError::Internal { error: Report::new(e) }
	}
}

impl From<ErrReport> for ServerError {
	fn from(e: ErrReport) -> ServerError {
		ServerError::Internal { error: e }
	}
}

impl From<serde_json::Error> for ServerError {
	fn from(e: serde_json::Error) -> ServerError {
		ServerError::Internal { error: Report::new(e) }
	}
}
