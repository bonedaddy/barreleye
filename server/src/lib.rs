use eyre::Result;
use std::sync::Arc;

pub use barreleye_common;

use barreleye_chain::Networks;
use barreleye_common::{
	progress, progress::Step, AppError, AppState, Clickhouse, Db, Env, Settings,
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

	let db = Db::new(settings.clone())
		.await
		.map_err(|url| {
			progress::quit(AppError::DatabaseConnection {
				url: url.to_string(),
			});
		})
		.unwrap();
	db.run_migrations().await?;
	let database = Arc::new(db);

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
