use eyre::Result;
use sea_orm::DatabaseConnection;
use std::sync::{atomic::AtomicBool, Arc};
use uuid::Uuid;

pub use barreleye_common;

use barreleye_chain::Networks;
use barreleye_common::{
	db, progress, progress::Step, utils, AppError, Clickhouse, Env, Settings,
};
use errors::ServerError;

mod errors;
mod handlers;
mod lists;
mod server;

#[derive(Clone)]
pub struct ServerState {
	pub uuid: Uuid,
	pub is_leader: Arc<AtomicBool>,
	pub settings: Arc<Settings>,
	pub warehouse: Arc<Clickhouse>,
	pub db: Arc<DatabaseConnection>,
	pub networks: Arc<Networks>,
	pub env: Env,
}

impl ServerState {
	pub fn new(
		settings: Arc<Settings>,
		warehouse: Arc<Clickhouse>,
		db: Arc<DatabaseConnection>,
		networks: Arc<Networks>,
		env: Env,
	) -> Self {
		ServerState {
			uuid: utils::new_uuid(),
			is_leader: Arc::new(AtomicBool::new(false)),
			settings,
			warehouse,
			db,
			networks,
			env,
		}
	}
}

pub type ServerResult<T> = Result<T, ServerError>;

#[tokio::main]
pub async fn start(env: Env) -> Result<()> {
	progress::show(Step::Setup).await;

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
	db::run_migrations(&db_conn).await?;
	let database = Arc::new(db_conn);

	let networks = Arc::new(Networks::new(database.clone(), env).await?);
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
			).await
		}),
		tokio::spawn(async move {
			networks_clone.watch().await
		}),
		tokio::spawn(async move {
			lists.watch().await
		}),
	};

	server_done.and(watcher_done).and(lists_done)?;

	Ok(())
}
