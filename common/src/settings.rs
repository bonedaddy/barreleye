use config::Config;
use eyre::Result;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Server {
	pub ip_v4: String,
	pub ip_v6: String,
	pub port: u16,
}

#[derive(Debug, Deserialize)]
pub struct DatabaseType {
	pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct Database {
	pub driver: String,
	pub name: String,
	pub min_connections: u32,
	pub max_connections: u32,
	pub connect_timeout: u64,
	pub idle_timeout: u64,
	pub max_lifetime: u64,
	pub sqlite: DatabaseType,
	pub postgres: DatabaseType,
	pub mysql: DatabaseType,
}

#[derive(Debug, Deserialize)]
pub struct Settings {
	pub server: Server,
	pub database: Database,
}

impl Settings {
	pub fn new() -> Result<Self> {
		let s = Config::builder()
			.add_source(config::File::with_name("settings"))
			.add_source(config::Environment::with_prefix("BARRELEYE"))
			.build()?;

		let settings = s.try_deserialize()?;
		Ok(settings)
	}
}
