use clap::{arg, command, value_parser};
use eyre::Result;
use std::{env, sync::Arc};
use tokio::{
	signal,
	time::{sleep, Duration},
};
use uuid::Uuid;

use crate::lists::Lists;
use barreleye_chain::Networks;
use barreleye_common::{
	models::{Config, ConfigKey},
	progress,
	progress::Step,
	utils, AppError, AppState, Cache, Db, Env, Settings, Warehouse,
};
use barreleye_server::Server;

mod banner;
mod lists;
mod log;

#[tokio::main]
async fn main() -> Result<()> {
	log::setup()?;

	let matches = command!()
		.author("Barreleye")
		.version(env!("CARGO_PKG_VERSION"))
		.propagate_version(true)
		.arg(
			arg!(-e --env <ENV> "Network types to connect to")
				.value_parser(value_parser!(Env)),
		)
		.arg(arg!(--indexer "Run only indexer, without the server"))
		.arg(arg!(--server "Run only server, without the indexer"))
		.arg(arg!(-p --plain "No ASCII banner"))
		.get_matches();

	let env: Env = *matches.get_one("env").unwrap_or(&Env::Mainnet);
	let skip_ascii: bool = *matches.get_one("plain").unwrap_or(&false);

	let (is_indexer, is_server) = match (
		*matches.get_one("indexer").unwrap_or(&false),
		*matches.get_one("server").unwrap_or(&false),
	) {
		(true, _) => (true, false),
		(_, true) => (false, true),
		_ => (true, true),
	};

	banner::show(env, is_indexer, is_server, skip_ascii)?;
	progress::show(Step::Setup).await;

	let settings = Arc::new(Settings::new()?);

	let cache = Arc::new(
		Cache::new(settings.clone())
			.await
			.map_err(|url| {
				progress::quit(AppError::ServiceConnection {
					service: settings.cache.driver.to_string(),
					url: url.to_string(),
				});
			})
			.unwrap(),
	);

	let warehouse = Arc::new(
		Warehouse::new(settings.clone())
			.await
			.unwrap()
			.run_migrations()
			.await
			.map_err(|url| {
				progress::quit(AppError::ServiceConnection {
					service: settings.warehouse.driver.to_string(),
					url: url.to_string(),
				});
			})
			.unwrap(),
	);

	let db = Arc::new(
		Db::new(settings.clone())
			.await
			.map_err(|url| {
				progress::quit(AppError::ServiceConnection {
					service: settings.db.driver.to_string(),
					url: url.to_string(),
				});
			})
			.unwrap()
			.run_migrations()
			.await?,
	);

	let app_state = Arc::new(AppState::new(
		settings, cache, db, warehouse, env, is_indexer, is_server,
	));

	let mut networks = Networks::new(app_state.clone()).connect().await?;
	let server = Server::new(app_state.clone());
	let lists = Lists::new(app_state.clone());

	let (server_done, watcher_done, lists_done, _, _) = tokio::join! {
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
				true => tokio::select! {
					v = networks.watch() => v,
					_ = signal::ctrl_c() => Ok(()),
				},
				_ => Ok(())
			}
		}),
		tokio::spawn(async move {
			match is_indexer {
				true => tokio::select! {
					v = lists.watch() => v,
					_ = signal::ctrl_c() => Ok(()),
				},
				_ => Ok(())
			}
		}),
		tokio::spawn({
			let app_state = app_state.clone();
			async move {
				match is_indexer {
					true => tokio::select! {
						v = leader_check(app_state) => v,
						_ = signal::ctrl_c() => Ok(()),
					},
					_ => Ok(())
				}
			}
		}),
		tokio::spawn(async {
			signal::ctrl_c().await.ok();
			println!("\nSIGINT received; bye 👋");
		}),
	};

	server_done.and(watcher_done).and(lists_done)?
}

async fn leader_check(app_state: Arc<AppState>) -> Result<()> {
	let leader_ping = app_state.settings.leader_ping;
	let leader_promotion = app_state.settings.leader_promotion;

	if app_state.is_indexer && !app_state.is_server {
		progress::show(Step::IndexerReady).await;
	}

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
