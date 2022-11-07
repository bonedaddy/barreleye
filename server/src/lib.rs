use eyre::Result;
use sea_orm::DatabaseConnection;
use std::sync::Arc;

pub use barreleye_common;

use barreleye_chain::Networks;
use barreleye_common::{
	db, progress, progress::Step, AppError, Clickhouse, Env, Settings,
};
use error::ServerError;

mod error;
mod handlers;
mod lists;
mod server;

#[derive(Clone)]
pub struct ServerState {
	pub settings: Arc<Settings>,
	pub warehouse: Arc<Clickhouse>,
	pub db: Arc<DatabaseConnection>,
	pub networks: Option<Arc<Networks>>,
	pub env: Env,
	is_watcher: bool,
}

impl ServerState {
	pub fn new(
		settings: Arc<Settings>,
		warehouse: Arc<Clickhouse>,
		db: Arc<DatabaseConnection>,
		networks: Option<Arc<Networks>>,
		env: Env,
		is_watcher: bool,
	) -> Self {
		ServerState { settings, warehouse, db, networks, env, is_watcher }
	}

	pub fn is_watcher(&self) -> bool {
		self.is_watcher
	}
}

pub type ServerResult<T> = Result<T, ServerError>;

#[tokio::main]
pub async fn start(env: Env, is_watcher: bool) -> Result<()> {
	progress::show(Step::Setup, is_watcher).await;

	let settings = Arc::new(Settings::new()?);

	let clickhouse = Clickhouse::new(settings.clone())
		.await
		.map_err(|_| {
			progress::quit(AppError::WarehouseConnection {
				url: settings.warehouse.clickhouse.url.clone(),
			});
		})
		.unwrap();
	let warehouse = Arc::new(clickhouse);

	let db_conn = db::new(settings.clone())
		.await
		.map_err(|_| {
			progress::quit(AppError::DatabaseConnection {
				url: db::get_url(settings.clone()),
			});
		})
		.unwrap();
	db::run_migrations(&db_conn, is_watcher).await?;
	let database = Arc::new(db_conn);

	let mut networks = None;
	if is_watcher {
		networks = Some(Arc::new(
			Networks::new(database.clone(), env, is_watcher).await?,
		));
	}
	let networks_clone = networks.clone();

	let lists = lists::Lists::new(database.clone(), settings.clone());

	let (server_done, watcher_done, lists_done) = tokio::join! {
		tokio::spawn(async move {
			server::start(
				settings,
				warehouse,
				database,
				networks,
				env,
				is_watcher,
			).await
		}),
		tokio::spawn(async move {
			if is_watcher {
				networks_clone.unwrap().watch().await
			}
		}),
		tokio::spawn(async move {
			if is_watcher {
				lists.watch().await
			}
		}),
	};

	server_done.and(watcher_done).and(lists_done)?;

	Ok(())
}
