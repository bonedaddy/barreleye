use derive_more::{Display, Error};

#[derive(Debug, Display, Error)]
pub enum AppError {
	#[display(fmt = "Failed to install signal handler.")]
	SignalHandler,

	#[display(fmt = "Invalid setting for `{key}`: `{value}`.")]
	InvalidSetting { key: String, value: String },

	#[display(fmt = "Could not connect to the warehouse database at `{url}`.")]
	WarehouseConnection { url: String },

	#[display(fmt = "Could not connect to the database at `{url}`.")]
	DatabaseConnection { url: String },

	#[display(fmt = "Could not complete network setup:\n{error}")]
	NetworkFailure { error: String },

	#[display(
		fmt = "Promotion timeout should be at least 2x processing frequency."
	)]
	InvalidPromotionTimeout,
}
