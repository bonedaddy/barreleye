use derive_more::{Display, Error};

#[derive(Debug, Clone, Display, Error)]
pub enum AppError<'a> {
	#[display(fmt = "Failed to install signal handler")]
	SignalHandler,

	#[display(fmt = "Invalid config @ `{config}`: {error}")]
	Config { config: &'a str, error: &'a str },

	#[display(fmt = "Could not start server @ `{url}`: {error}")]
	ServerStartup { url: &'a str, error: &'a str },

	#[display(
		fmt = "Barreleye requires Clickhouse to run. Provide the connection URL with \"--warehouse=<URL>\". Could not connect to warehouse @ `{url}`"
	)]
	WarehouseConnection { url: &'a str },

	#[display(fmt = "Could not connect to database @ `{url}`")]
	DatabaseConnection { url: &'a str },

	#[display(fmt = "Could not complete network setup:\n{error}")]
	Network { error: &'a str },

	#[display(fmt = "Indexing failed: {error}")]
	Indexing { error: &'a str },

	#[display(fmt = "Unexpected error: {error}")]
	Unexpected { error: &'a str },
}
