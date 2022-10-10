use eyre::Result;
use sea_orm::DatabaseConnection;

pub mod constants;
pub mod db;
pub mod errors;
pub mod models;
pub mod settings;
pub use settings::Settings;
pub mod utils;

pub struct AppState {
	pub db: DatabaseConnection,
}

pub type ServerResult<T> = Result<T, errors::AppError>;
