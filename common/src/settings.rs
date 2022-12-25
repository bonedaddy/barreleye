use config::{Config, Environment, File};
use directories::BaseDirs;
use eyre::Result;
use serde::Deserialize;
use std::{env::var, fs, fs::OpenOptions, path::PathBuf};
use url::Url;

use crate::{
	cache::Driver as CacheDriver, db::Driver as DatabaseDriver, errors::AppError, quit, utils,
	warehouse::Driver as WarehouseDriver,
};

pub static DEFAULT_SETTINGS_FILENAME: &str = "barreleye.toml";
pub static DEFAULT_SETTINGS_CONTENT: &str = r#"
sdn_refresh_rate = 3600 # in seconds
primary_ping = 2 # in seconds
primary_promotion = 20 # in seconds

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
	pub primary_ping: u64,
	pub primary_promotion: u64,
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
	pub fn new(config_path: Option<String>) -> Result<Self> {
		// figure out the config path
		let config_path = {
			let mut ret = None;

			// try custom a config file (if provided)
			if let Some(filename) = config_path {
				let try_filename = PathBuf::from(filename.clone());
				if try_filename.exists() {
					ret = Some(try_filename);
				} else {
					quit(AppError::MissingConfigFile { filename });
				}
			} else {
				// load a few places to check and/or create
				let mut paths = vec![];
				if let Ok(manifest_dir) = var("CARGO_MANIFEST_DIR") {
					paths.push(PathBuf::from(manifest_dir).join(DEFAULT_SETTINGS_FILENAME))
				}
				paths.push(std::env::current_exe()?.join(DEFAULT_SETTINGS_FILENAME));
				if let Some(base_dir) = BaseDirs::new() {
					paths.push(
						PathBuf::from(base_dir.config_dir())
							.join("barreleye")
							.join(DEFAULT_SETTINGS_FILENAME),
					);
				}

				// check if any of those paths exist
				for path in paths.clone().into_iter() {
					if path.exists() {
						ret = Some(path);
						break;
					}
				}

				// if none found, try to create a default config
				if ret.is_none() {
					for path in paths.into_iter() {
						if OpenOptions::new()
							.write(true)
							.create_new(true)
							.open(path.clone())
							.is_ok()
						{
							fs::write(path.clone(), DEFAULT_SETTINGS_CONTENT.trim())?;

							ret = Some(path);
							break;
						}
					}
				}

				// if still nothing, we failed
				if ret.is_none() {
					quit(AppError::DefaultConfigFile);
				}
			}

			ret.unwrap()
		};

		// builder settings
		let s = Config::builder()
			.add_source(File::from(config_path).required(false))
			.add_source(Environment::with_prefix("BARRELEYE"));

		// try to create a struct
		let settings: Settings = s.build()?.try_deserialize()?;

		// test: dsn for cache
		if settings.cache.driver == CacheDriver::RocksDB &&
			utils::get_db_path(&settings.dsn.rocksdb).is_empty()
		{
			quit(AppError::InvalidSetting {
				key: "dsn.rocksdb".to_string(),
				value: settings.dsn.rocksdb.clone(),
			});
		}

		// test: dsn for warehouse
		if settings.warehouse.driver == WarehouseDriver::Clickhouse &&
			Url::parse(&settings.dsn.clickhouse).is_err()
		{
			quit(AppError::InvalidSetting {
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
			quit(AppError::InvalidSetting {
				key: format!("dsn.{}", settings.db.driver),
				value: db_url,
			});
		}

		// test: primary settings
		if settings.primary_promotion < settings.primary_ping * 3 {
			quit(AppError::InvalidPrimaryConfigs);
		}

		Ok(settings)
	}
}
