use config::{Config, Environment, File, FileFormat};
use directories::BaseDirs;
use eyre::Result;
use serde::Deserialize;
use std::{fs, fs::OpenOptions, path::PathBuf};
use url::Url;

use crate::{
	cache::Driver as CacheDriver, db::Driver as DatabaseDriver, errors::AppError, progress, utils,
	warehouse::Driver as WarehouseDriver,
};

pub static DEFAULT_SETTINGS_FILENAME: &str = "barreleye.toml";
pub static DEFAULT_SETTINGS_CONTENT: &str = r#"
sdn_refresh_rate = 3600 # in seconds
leader_ping = 5 # in seconds
leader_promotion = 15 # in seconds

[server]
ip_v4 = "0.0.0.0"
ip_v6 = "::"
port = 22775

[cache]
driver = "rocksdb"

[db]
driver = "sqlite" # or "postgres" or "mysql"
min_connections = 5
max_connections = 100
connect_timeout = 8
idle_timeout = 8
max_lifetime = 8

[warehouse]
driver = "clickhouse"

[dsn]
rocksdb = "rocksdb://barreleye_cache"
sqlite = "sqlite://barreleye_database?mode=rwc"
postgres = "" # eg: "postgres://USERNAME[:PASSWORD]@localhost:5432/database"
mysql = "" # eg: "mysql://USERNAME[:PASSWORD]@localhost:3306/database"
clickhouse = "" # eg: "http://USERNAME[:PASSWORD]@localhost:8123/database"
"#;

#[derive(Debug, Deserialize)]
pub struct Settings {
	pub sdn_refresh_rate: u64,
	pub leader_ping: u64,
	pub leader_promotion: u64,
	pub server: Server,
	pub cache: Cache,
	pub db: Db,
	pub warehouse: Warehouse,
	pub dsn: Dsn,
}

#[derive(Debug, Deserialize)]
pub struct Server {
	pub ip_v4: String,
	pub ip_v6: String,
	pub port: u16,
}

#[derive(Debug, Deserialize)]
pub struct Cache {
	pub driver: CacheDriver,
}

#[derive(Debug, Deserialize)]
pub struct Db {
	pub driver: DatabaseDriver,
	pub min_connections: u32,
	pub max_connections: u32,
	pub connect_timeout: u64,
	pub idle_timeout: u64,
	pub max_lifetime: u64,
}

#[derive(Debug, Deserialize)]
pub struct Warehouse {
	pub driver: WarehouseDriver,
}

#[derive(Debug, Deserialize)]
pub struct Dsn {
	pub rocksdb: String,
	pub sqlite: String,
	pub postgres: String,
	pub mysql: String,
	pub clickhouse: String,
}

impl Settings {
	pub fn new() -> Result<Self> {
		// create a blank file if doesn't exist
		if OpenOptions::new().write(true).create_new(true).open(DEFAULT_SETTINGS_FILENAME).is_ok() {
			fs::write(DEFAULT_SETTINGS_FILENAME, DEFAULT_SETTINGS_CONTENT.trim())?;
		}

		// builder settings
		let mut s = Config::builder()
			.add_source(File::new(DEFAULT_SETTINGS_FILENAME, FileFormat::Toml).required(false));

		if let Some(dir) = BaseDirs::new() {
			s = s.add_source(
				File::from(
					PathBuf::from(dir.config_dir())
						.join("barreleye")
						.join(DEFAULT_SETTINGS_FILENAME),
				)
				.required(false),
			);
		}

		s = s.add_source(Environment::with_prefix("BARRELEYE"));

		// try to create a struct
		let settings: Settings = s.build()?.try_deserialize()?;

		// test: dsn for cache
		if settings.cache.driver == CacheDriver::RocksDB &&
			utils::get_db_path(&settings.dsn.rocksdb).is_empty()
		{
			progress::quit(AppError::InvalidSetting {
				key: "dsn.rocksdb".to_string(),
				value: settings.dsn.rocksdb.clone(),
			});
		}

		// test: dsn for warehouse
		if settings.warehouse.driver == WarehouseDriver::Clickhouse &&
			Url::parse(&settings.dsn.clickhouse).is_err()
		{
			progress::quit(AppError::InvalidSetting {
				key: "dsn.clickhouse".to_string(),
				value: settings.dsn.clickhouse.clone(),
			});
		}

		// test: dsn for db
		let db_url = match settings.db.driver {
			DatabaseDriver::SQLite => settings.dsn.sqlite.clone(),
			DatabaseDriver::PostgreSQL => settings.dsn.postgres.clone(),
			DatabaseDriver::MySQL => settings.dsn.mysql.clone(),
		};
		if Url::parse(&db_url).is_err() {
			progress::quit(AppError::InvalidSetting {
				key: format!("dsn.{}", settings.db.driver),
				value: db_url,
			});
		}

		// test: leader settings
		if settings.leader_ping * 2 >= settings.leader_promotion {
			progress::quit(AppError::InvalidLeaderConfigs);
		}

		Ok(settings)
	}
}
