use axum::{
	http::StatusCode,
	response::{IntoResponse, Response},
	Json,
};
use derive_more::{Display, Error};
use eyre::ErrReport;
use serde_json::json;

#[derive(Debug, Display, Error)]
pub enum AppError {
	Internal {
		error: String,
	},

	#[display(fmt = "Invalid setting \"{key}\" = `{value}`")]
	Settings {
		key: String,
		value: String,
	},
}

impl From<ErrReport> for AppError {
	fn from(err: ErrReport) -> AppError {
		AppError::Internal { error: err.to_string() }
	}
}

impl IntoResponse for AppError {
	fn into_response(self) -> Response {
		let (status, error_message) = match self {
			AppError::Settings { key: _, value: _ } => {
				(StatusCode::INTERNAL_SERVER_ERROR, "Invalid settings")
			}
			_ => (StatusCode::INTERNAL_SERVER_ERROR, "Something broke"),
		};

		let body = Json(json!({
			"error": error_message,
		}));

		(status, body).into_response()
	}
}
