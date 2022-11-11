use eyre::Result;
use std::sync::Arc;
use tokio::{
	signal,
	time::{sleep, Duration},
};
use uuid::Uuid;

use barreleye_chain::Networks;
use barreleye_common::{
	models::{Cache, CacheKey},
	progress,
	progress::Step,
	utils, AppError, AppState, Clickhouse, Db, Env, Settings,
};
use errors::ServerError;
use lists::Lists;
use server::Server;

mod errors;
mod handlers;
mod lists;
mod server;

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

	let db = Arc::new(
		Db::new(settings.clone())
			.await
			.map_err(|url| {
				progress::quit(AppError::DatabaseConnection {
					url: url.to_string(),
				});
			})
			.unwrap()
			.run_migrations()
			.await?,
	);

	let app_state = Arc::new(AppState::new(settings, warehouse, db, env));

	let mut networks = Networks::new(app_state.clone()).connect().await?;
	let server = Server::new(app_state.clone());
	let lists = Lists::new(app_state.clone());

	let (server_done, watcher_done, lists_done, _) = tokio::join! {
		tokio::spawn(async move {
			server.start().await
		}),
		tokio::spawn(async move {
			networks.watch().await
		}),
		tokio::spawn(async move {
			lists.watch().await
		}),
		tokio::spawn({
			let app_state = app_state.clone();
			async move {
				tokio::select! {
					_ = leader_check(app_state) => {},
					_ = signal::ctrl_c() => {},
				}
			}
		}),
	};

	server_done.and(watcher_done).and(lists_done)?
}

async fn leader_check(app_state: Arc<AppState>) -> Result<()> {
	let frequency = app_state.settings.warehouse.processing_frequency;
	let timeout = app_state.settings.warehouse.leader_promotion_timeout;

	loop {
		let active_at = utils::ago_in_seconds(frequency + 1);
		let promoted_at = utils::ago_in_seconds(timeout);

		let check_in = Cache::set::<Uuid>(
			&app_state.db,
			CacheKey::Leader.into(),
			app_state.uuid,
		);

		match Cache::get::<Uuid>(&app_state.db, CacheKey::Leader.into()).await?
		{
			None => {
				check_in.await?;
			}
			Some(hit)
				if hit.value == app_state.uuid &&
					hit.updated_at >= active_at =>
			{
				check_in.await?;
				app_state.set_is_leader(true);
			}
			Some(hit) if hit.updated_at < promoted_at => {
				check_in.await?;
			}
			_ => {
				app_state.set_is_leader(false);
			}
		}

		sleep(Duration::from_secs(frequency)).await
	}
}
