use eyre::Result;
use std::sync::Arc;

pub use barreleye_common;

use barreleye_chain::Networks;
use barreleye_common::{
	db, progress, progress::Step, AppError, AppState, Clickhouse, Env, Settings,
};
use errors::ServerError;

mod errors;
mod handlers;

mod lists;
use lists::Lists;

mod server;
use server::Server;

pub type ServerResult<T> = Result<T, ServerError>;

#[tokio::main]
pub async fn start(env: Env) -> Result<()> {
	progress::show(Step::Setup).await;

	let settings = Arc::new(Settings::new()?);

	let warehouse = Arc::new(
		Clickhouse::new(settings.clone())
			.await
			.map_err(|_| {
				progress::quit(AppError::WarehouseConnection {
					url: settings.warehouse.clickhouse.url.clone(),
				});
			})
			.unwrap(),
	);

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

	let app_state = Arc::new(AppState::new(settings, warehouse, database, env));

	let mut networks = Networks::new(app_state.clone());
	networks.connect().await?;

	let (server_done, watcher_done, lists_done) = tokio::join! {
		tokio::spawn({
			let app_state = app_state.clone();
			async move {
				Server::new(app_state).start().await
			}
		}),
		tokio::spawn(async move {
			networks.start().await
		}),
		tokio::spawn({
			let app_state = app_state.clone();
			async move {
				Lists::new(app_state).start().await
			}
		}),
	};

	server_done.and(watcher_done).and(lists_done)?;

	Ok(())
}
