use derive_more::Display;
use eyre::{bail, Result};
use log::LevelFilter;
use sea_orm::{
	ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend,
	Statement,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use url::Url;

use crate::{errors::AppError, progress, progress::Step, Settings};

#[derive(Display, Debug, Serialize, Deserialize)]
pub enum Backend {
	#[display(fmt = "sqlite")]
	#[serde(rename = "sqlite")]
	SQLite,

	#[display(fmt = "postgres")]
	#[serde(rename = "postgres")]
	PostgreSQL,

	#[display(fmt = "mysql")]
	#[serde(rename = "mysql")]
	MySQL,
}

mod migrations;
use migrations::{Migrator, MigratorTrait};

pub async fn new() -> Result<DatabaseConnection> {
	let settings = Settings::new()?;

	progress::show(Step::Database).await;

	let url = match settings.database.backend {
		Backend::SQLite => settings.database.sqlite.url,
		Backend::PostgreSQL => settings.database.postgres.url,
		Backend::MySQL => settings.database.mysql.url,
	};

	let with_options = |url: String| -> ConnectOptions {
		let mut opt = ConnectOptions::new(url);

		// @TODO for sqlite, max out at 1 connection otherwise
		// writes are not guaranteed to be executed serially
		let (min_connections, max_connections) = match settings.database.backend
		{
			Backend::SQLite => (1, 1),
			_ => (
				settings.database.min_connections,
				settings.database.max_connections,
			),
		};

		opt.max_connections(max_connections)
			.min_connections(min_connections)
			.connect_timeout(Duration::from_secs(
				settings.database.connect_timeout,
			))
			.idle_timeout(Duration::from_secs(settings.database.idle_timeout))
			.max_lifetime(Duration::from_secs(settings.database.max_lifetime))
			.sqlx_logging(false)
			.sqlx_logging_level(LevelFilter::Warn);

		opt
	};

	let err_settings_database_url = AppError::Settings {
		key: format!("database.{}.backend", settings.database.backend),
		value: url.clone(),
	};
	if Url::parse(&url).is_err() {
		bail!(err_settings_database_url);
	}
	let conn = Database::connect(with_options(url.clone()))
		.await
		.or_else(|_| bail!(err_settings_database_url))?;

	let db_name = settings.database.name;
	let url_with_database = format!("{url}/{db_name}");

	let db = match conn.get_database_backend() {
		DbBackend::MySql => {
			conn.execute(Statement::from_string(
				DbBackend::MySql,
				format!("CREATE DATABASE IF NOT EXISTS `{db_name}`;"),
			))
			.await?;

			Database::connect(with_options(url_with_database)).await?
		}
		DbBackend::Postgres => {
			let result = conn
				.execute(Statement::from_string(DbBackend::Postgres, format!("SELECT datname FROM pg_catalog.pg_database WHERE datname='{db_name}';")))
				.await?;

			if result.rows_affected() == 0 {
				conn.execute(Statement::from_string(
					DbBackend::Postgres,
					format!(r#"CREATE DATABASE "{db_name}";"#),
				))
				.await?;
			}

			Database::connect(with_options(url_with_database)).await?
		}
		DbBackend::Sqlite => conn,
	};

	progress::show(Step::Migrations).await;
	Migrator::up(&db, None).await?;

	Ok(db)
}
