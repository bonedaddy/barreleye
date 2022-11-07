use derive_more::{Display, Error};

#[derive(Debug, Display, Error)]
pub enum AppError {
	#[display(fmt = "Failed to install signal handler.")]
	SignalHandler,

	#[display(
		fmt = "Check configuration settings. Invalid value found for `{key} = {value}`."
	)]
	InvalidSetting { key: String, value: String },

	#[display(
		fmt = "Could not connect to the warehouse database at `{url}`. Check `warehouse` settings and make sure the server is accessible."
	)]
	WarehouseConnection { url: String },

	#[display(
		fmt = "Could not connect to the database at `{url}`. Check `database` settings and make sure the server is accessible."
	)]
	DatabaseConnection { url: String },

	#[display(fmt = "Could not complete network setup:\n{error}")]
	NetworkFailure { error: String },
}
