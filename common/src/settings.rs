use config::{Config, Environment, File, FileFormat};
use eyre::Result;
use serde::Deserialize;
use std::{fs, fs::OpenOptions};
use url::Url;

use crate::{db::Dialect as DatabaseDialect, errors::AppError, progress};

pub static DEFAULT_SETTINGS_FILENAME: &str = "settings.toml";
pub static DEFAULT_SETTINGS_CONTENT: &str = r#"
hardcoded_lists_refresh_rate = 3600 # in seconds

[server]
ip_v4 = "0.0.0.0"
ip_v6 = "" # "::"
port = 22775

[warehouse]
dialect = "clickhouse"
name = "barreleye"
processing_frequency = 5 # in seconds
leader_promotion_timeout = 15 # in seconds

[warehouse.clickhouse]
url = "http://localhost:8123"

[database]
dialect = "sqlite"
name = "barreleye"
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
pub struct Dsn {
	pub url: String,
}

#[derive(Debug, Deserialize)]
pub enum WarehouseDialect {
	#[serde(rename = "clickhouse")]
	Clickhouse,
}

#[derive(Debug, Deserialize)]
pub struct Warehouse {
	pub dialect: WarehouseDialect,
	pub name: String,
	pub processing_frequency: u64,
	pub leader_promotion_timeout: u64,
	pub clickhouse: Dsn,
}

#[derive(Debug, Deserialize)]
pub struct Database {
	pub dialect: DatabaseDialect,
	pub name: String,
	pub min_connections: u32,
	pub max_connections: u32,
	pub connect_timeout: u64,
	pub idle_timeout: u64,
	pub max_lifetime: u64,
	pub sqlite: Dsn,
	pub postgres: Dsn,
	pub mysql: Dsn,
}

#[derive(Debug, Deserialize)]
pub struct Settings {
	pub hardcoded_lists_refresh_rate: u64,
	pub server: Server,
	pub warehouse: Warehouse,
	pub database: Database,
}

impl Settings {
	pub fn new() -> Result<Self> {
		// create a blank file if doesn't exist
		if OpenOptions::new()
			.write(true)
			.create_new(true)
			.open(DEFAULT_SETTINGS_FILENAME)
			.is_ok()
		{
			fs::write(
				DEFAULT_SETTINGS_FILENAME,
				DEFAULT_SETTINGS_CONTENT.trim(),
			)?;
		}

		// builder settings
		let s = Config::builder()
			.add_source(File::new(DEFAULT_SETTINGS_FILENAME, FileFormat::Toml))
			.add_source(Environment::with_prefix("BARRELEYE"))
			.build()?;

		// try to create a struct
		let settings: Settings = s.try_deserialize()?;

		// test for common errors
		if Url::parse(&settings.warehouse.clickhouse.url).is_err() {
			progress::quit(AppError::InvalidSetting {
				key: "warehouse.clickhouse.url".to_string(),
				value: settings.warehouse.clickhouse.url.clone(),
			});
		}

		let backend_url = match settings.database.dialect {
			DatabaseDialect::SQLite => settings.database.sqlite.url.clone(),
			DatabaseDialect::PostgreSQL => {
				settings.database.postgres.url.clone()
			}
			DatabaseDialect::MySQL => settings.database.mysql.url.clone(),
		};
		if Url::parse(&backend_url).is_err() {
			progress::quit(AppError::InvalidSetting {
				key: format!("database.{}.url", settings.database.dialect),
				value: backend_url,
			});
		}

		if settings.warehouse.processing_frequency * 2 >=
			settings.warehouse.leader_promotion_timeout
		{
			progress::quit(AppError::InvalidPromotionTimeout);
		}

		Ok(settings)
	}
}
