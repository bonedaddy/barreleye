use derive_more::Display;
use eyre::{Result, WrapErr};
use log::LevelFilter;
use sea_orm::{
	ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend,
	Statement,
};
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::Duration};

use crate::{progress, progress::Step, utils, Settings};
use migrations::{Migrator, MigratorTrait};

mod migrations;

#[derive(Display, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum Driver {
	#[display(fmt = "SQLite")]
	#[serde(rename = "sqlite")]
	SQLite,

	#[display(fmt = "PostgreSQL")]
	#[serde(rename = "postgres")]
	PostgreSQL,

	#[display(fmt = "MySQL")]
	#[serde(rename = "mysql")]
	MySQL,
}

pub struct Db {
	db: DatabaseConnection,
}

impl Db {
	pub async fn new(settings: Arc<Settings>) -> Result<Self> {
		let url = match settings.db.driver {
			Driver::SQLite => settings.dsn.sqlite.clone(),
			Driver::PostgreSQL => settings.dsn.postgres.clone(),
			Driver::MySQL => settings.dsn.mysql.clone(),
		};

		let with_options = |url: String| -> ConnectOptions {
			let mut opt = ConnectOptions::new(url);

			// @TODO for sqlite, max out at 1 connection otherwise
			// writes are not guaranteed to be executed serially
			let (min_connections, max_connections) = match settings.db.driver {
				Driver::SQLite => (1, 1),
				_ => (settings.db.min_connections, settings.db.max_connections),
			};

			opt.max_connections(max_connections)
				.min_connections(min_connections)
				.connect_timeout(Duration::from_secs(
					settings.db.connect_timeout,
				))
				.idle_timeout(Duration::from_secs(settings.db.idle_timeout))
				.max_lifetime(Duration::from_secs(settings.db.max_lifetime))
				.sqlx_logging(false)
				.sqlx_logging_level(LevelFilter::Warn);

			opt
		};

		let (url_without_database, db_name) = utils::without_pathname(&url);
		let url_with_database = url;

		let conn =
			Database::connect(with_options(url_without_database.clone()))
				.await
				.wrap_err(url_without_database.clone())?;

		let db = match conn.get_database_backend() {
			DbBackend::MySql => {
				conn.execute(Statement::from_string(
					DbBackend::MySql,
					format!("CREATE DATABASE IF NOT EXISTS `{db_name}`;"),
				))
				.await
				.wrap_err(url_without_database.clone())?;

				Database::connect(with_options(url_with_database.clone()))
					.await
					.wrap_err(url_with_database.clone())?
			}
			DbBackend::Postgres => {
				let result = conn
					.execute(Statement::from_string(DbBackend::Postgres, format!("SELECT datname FROM pg_catalog.pg_database WHERE datname='{db_name}';")))
					.await.wrap_err(url_without_database.clone())?;

				if result.rows_affected() == 0 {
					conn.execute(Statement::from_string(
						DbBackend::Postgres,
						format!(r#"CREATE DATABASE "{db_name}";"#),
					))
					.await
					.wrap_err(url_without_database.clone())?;
				}

				Database::connect(with_options(url_with_database.clone()))
					.await
					.wrap_err(url_with_database.clone())?
			}
			DbBackend::Sqlite => conn,
		};

		Ok(Self { db })
	}

	pub async fn run_migrations(self) -> Result<Self> {
		progress::show(Step::Migrations).await;
		Migrator::up(&self.db, None).await?;

		Ok(self)
	}

	pub fn get(&self) -> &DatabaseConnection {
		&self.db
	}
}
