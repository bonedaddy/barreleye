use eyre::Result;
use std::sync::Arc;
use tokio::{
	signal,
	time::{sleep, Duration},
};
use uuid::Uuid;

use barreleye_chain::Networks;
use barreleye_common::{
	models::{Config, ConfigKey},
	progress,
	progress::Step,
	utils, AppError, AppState, Cache, Db, Env, Settings, Warehouse,
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
pub async fn start(env: Env, is_indexer: bool, is_server: bool) -> Result<()> {
	progress::show(Step::Setup).await;

	let settings = Arc::new(Settings::new()?);

	let warehouse = Arc::new(
		Warehouse::new(settings.clone())
			.await
			.map_err(|url| {
				progress::quit(AppError::WarehouseConnection {
					url: url.to_string(),
				});
			})
			.unwrap()
			.run_migrations()
			.await?,
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

	let cache = Arc::new(
		Cache::new(settings.clone())
			.await
			.map_err(|url| {
				progress::quit(AppError::CacheConnection {
					url: url.to_string(),
				});
			})
			.unwrap(),
	);

	let app_state = Arc::new(AppState::new(
		settings, cache, db, warehouse, env, is_indexer, is_server,
	));

	let mut networks = Networks::new(app_state.clone()).connect().await?;
	let server = Server::new(app_state.clone());
	let lists = Lists::new(app_state.clone());

	let (server_done, watcher_done, lists_done, _) = tokio::join! {
		tokio::spawn({
			let app_state = app_state.clone();
			async move {
				match is_server {
					true => server.start().await,
					_ => {
						app_state.set_is_ready();
						Ok(())
					}
				}
			}
		}),
		tokio::spawn(async move {
			match is_indexer {
				true => networks.watch().await,
				_ => Ok(())
			}
		}),
		tokio::spawn(async move {
			match is_indexer {
				true => lists.watch().await,
				_ => Ok(())
			}
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
	let leader_ping = app_state.settings.leader_ping;
	let leader_promotion = app_state.settings.leader_promotion;

	loop {
		let active_at = utils::ago_in_seconds(leader_ping + 1);
		let promoted_at = utils::ago_in_seconds(leader_promotion);

		let check_in = Config::set::<Uuid>(
			&app_state.db,
			ConfigKey::Leader,
			app_state.uuid,
		);

		match Config::get::<Uuid>(&app_state.db, ConfigKey::Leader).await? {
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

		sleep(Duration::from_secs(leader_ping)).await
	}
}
