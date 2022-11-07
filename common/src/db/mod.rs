use derive_more::Display;
use eyre::Result;
use log::LevelFilter;
use sea_orm::{
	ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend,
	Statement,
};
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::Duration};

use crate::{progress, progress::Step, Settings};

#[derive(Display, Debug, Serialize, Deserialize)]
pub enum Dialect {
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

pub async fn new(settings: Arc<Settings>) -> Result<DatabaseConnection> {
	let url = get_url(settings.clone());

	let with_options = |url: String| -> ConnectOptions {
		let mut opt = ConnectOptions::new(url);

		// @TODO for sqlite, max out at 1 connection otherwise
		// writes are not guaranteed to be executed serially
		let (min_connections, max_connections) = match settings.database.dialect
		{
			Dialect::SQLite => (1, 1),
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

	let db_name = settings.database.name.clone();
	let url_with_database = format!("{url}/{db_name}");
	let conn = Database::connect(with_options(url.clone())).await?;

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

	Ok(db)
}

pub async fn run_migrations(
	db: &DatabaseConnection,
	is_watcher: bool,
) -> Result<()> {
	progress::show(Step::Migrations, is_watcher).await;
	Migrator::up(db, None).await?;

	Ok(())
}

pub fn get_url(settings: Arc<Settings>) -> String {
	match settings.database.dialect {
		Dialect::SQLite => settings.database.sqlite.url.clone(),
		Dialect::PostgreSQL => settings.database.postgres.url.clone(),
		Dialect::MySQL => settings.database.mysql.url.clone(),
	}
}
