use derive_more::{Display, Error};

#[derive(Debug, Display, Error)]
pub enum AppError {
	#[display(fmt = "Failed to install signal handler.")]
	SignalHandler,

	#[display(fmt = "Could not start server @ `{url}`: {error}")]
	ServerStartup { url: String, error: String },

	#[display(fmt = "Invalid setting for `{key}`: `{value}`.")]
	InvalidSetting { key: String, value: String },

	#[display(fmt = "Could not connect to `{url}`.")]
	ServiceConnection { url: String },

	#[display(fmt = "Could not complete network setup:\n{error}")]
	NetworkFailure { error: String },

	#[display(fmt = "Leader promotion should be at least 2x leader ping.")]
	InvalidLeaderConfigs,
}
