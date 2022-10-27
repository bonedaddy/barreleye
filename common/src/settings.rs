use config::{Config, Environment, File, FileFormat};
use eyre::{bail, Result};
use serde::Deserialize;
use std::{fs, fs::OpenOptions};

use crate::{db::Backend, errors::AppError};

pub static DEFAULT_SETTINGS_FILENAME: &str = "settings.toml";
pub static DEFAULT_SETTINGS_CONTENT: &str = r#"
[server]
ip_v4 = "0.0.0.0"
ip_v6 = "" # "::"
port = 22775

[database]
backend = "sqlite"
name = "barreleye_insights"
min_connections = 5
max_connections = 100
connect_timeout = 8
idle_timeout = 8
max_lifetime = 8

[database.sqlite]
url = "sqlite://data.db?mode=rwc"

[database.postgres]
url = "" # eg: "postgres://USERNAME[:PASSWORD]@localhost:5432"

[database.mysql]
url = "" # eg: "mysql://USERNAME[:PASSWORD]@localhost:3306"
"#;

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
	pub backend: Backend,
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
		if OpenOptions::new()
			.write(true)
			.create_new(true)
			.open(DEFAULT_SETTINGS_FILENAME)
			.is_ok()
		{
			if let Err(e) = fs::write(
				DEFAULT_SETTINGS_FILENAME,
				DEFAULT_SETTINGS_CONTENT.trim(),
			) {
				bail!(AppError::Internal { error: e.to_string() });
			}
		}

		let s = Config::builder()
			.add_source(File::new(DEFAULT_SETTINGS_FILENAME, FileFormat::Toml))
			.add_source(Environment::with_prefix("BARRELEYE"))
			.build()?;

		let settings = s.try_deserialize()?;
		Ok(settings)
	}
}
