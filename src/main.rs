use clap::{arg, command, value_parser};
use eyre::Result;
use std::{env, sync::Arc};
use tokio::{
	signal,
	sync::RwLock,
	time::{sleep, Duration},
};
use uuid::Uuid;

use crate::lists::Lists;
use barreleye_chain::Networks;
use barreleye_common::{
	models::{Config, ConfigKey},
	progress,
	progress::Step,
	utils, AppError, AppState, Cache, Db, Env, Settings, Verbosity, Warehouse,
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
		.arg(arg!(-e --env <ENV> "Network types to connect to").value_parser(value_parser!(Env)))
		.arg(arg!(--indexer "Run only indexer, without the server"))
		.arg(arg!(--server "Run only server, without the indexer"))
		.arg(arg!(-v --verbose "Show warnings and info"))
		.arg(arg!(-p --plain "Hide ASCII banner"))
		.get_matches();

	let env = *matches.get_one("env").unwrap_or(&Env::Mainnet);
	let skip_ascii = *matches.get_one("plain").unwrap_or(&false);
	let verbosity = match *matches.get_one("verbose").unwrap_or(&false) {
		true => Verbosity::Info,
		_ => Verbosity::Silent,
	};

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

	let cache = Arc::new(RwLock::new(
		Cache::new(settings.clone())
			.await
			.map_err(|url| {
				progress::quit(AppError::ServiceConnection {
					service: settings.cache.driver.to_string(),
					url: url.to_string(),
				});
			})
			.unwrap(),
	));

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
		settings, cache, db, warehouse, env, verbosity, is_indexer, is_server,
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
					data = networks.index() => {
						if data.is_err() {
							progress::quit(AppError::IndexingFailed {
								error: data.as_ref().unwrap_err().to_string(),
							});
						}

						data
					},
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
	let leader_promotion = app_state.settings.leader_promotion;

	if app_state.is_indexer && !app_state.is_server {
		progress::show(Step::IndexerReady).await;
	}

	loop {
		let db = &app_state.db;
		let cool_down_period = utils::ago_in_seconds(leader_promotion / 2);

		let last_leader = Config::get::<Uuid>(&app_state.db, ConfigKey::Leader).await?;
		match last_leader {
			None => {
				// first run ever
				Config::set::<Uuid>(db, ConfigKey::Leader, app_state.uuid).await?;
			}
			Some(hit) if hit.value == app_state.uuid && hit.updated_at >= cool_down_period => {
				// if leader, check-in only if cool-down period has not started yet ↑
				if Config::set_where::<Uuid>(db, ConfigKey::Leader, app_state.uuid, hit).await? {
					app_state.set_is_leader(true).await?;
				}
			}
			Some(hit) if utils::ago_in_seconds(leader_promotion) > hit.updated_at => {
				// attempt to take over as a leader (set is_leader on the next iteration)
				Config::set_where::<Uuid>(db, ConfigKey::Leader, app_state.uuid, hit).await?;
			}
			_ => {
				// either cool-down period has started or some other node is leading
				app_state.set_is_leader(false).await?;
			}
		}

		sleep(Duration::from_secs(app_state.settings.leader_ping)).await
	}
}
