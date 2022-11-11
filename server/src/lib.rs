use eyre::Result;
use std::sync::Arc;
use tokio::{
	signal,
	time::{sleep, Duration},
};

use barreleye_chain::Networks;
use barreleye_common::{
	models::{BasicModel, Leader},
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

	let networks = Networks::new(app_state.clone()).connect().await?;
	let server = Server::new(app_state.clone());
	let lists = Lists::new(app_state.clone());

	Leader::truncate(
		&app_state.db,
		31_557_600 +
			app_state.settings.warehouse.processing_frequency +
			app_state.settings.warehouse.leader_promotion_timeout,
	)
	.await?;

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

	server_done.and(watcher_done).and(lists_done)?;

	Ok(())
}

async fn leader_check(app_state: Arc<AppState>) -> Result<()> {
	loop {
		let frequency = app_state.settings.warehouse.processing_frequency;
		let promotion_at = utils::ago_in_seconds(
			app_state.settings.warehouse.leader_promotion_timeout,
		);

		match Leader::get_active(&app_state.db, frequency + 1).await? {
			Some(leader) if leader.uuid == app_state.uuid => {
				Leader::check_in(&app_state.db, app_state.uuid).await?;
				app_state.set_is_leader(true);
			}
			None => {
				let promote = Leader::create(
					&app_state.db,
					Leader::new_model(app_state.uuid),
				);

				match Leader::get_last(&app_state.db).await? {
					Some(leader) if leader.updated_at < promotion_at => {
						promote.await?;
					}
					None => {
						promote.await?;
					}
					_ => {}
				}
			}
			_ => {}
		}

		sleep(Duration::from_secs(frequency)).await
	}
}
