use clap::{arg, command, value_parser};
use eyre::Result;
use std::{env, sync::Arc};
use tokio::{signal, sync::RwLock, task::JoinSet};

use barreleye_common::{
	quit, App, AppError, Cache, Db, Env, Progress, ProgressStep, Settings, Verbosity, Warehouse,
};
use barreleye_indexer::Indexer;
use barreleye_server::Server;

mod banner;
mod log;

#[tokio::main]
async fn main() -> Result<()> {
	log::setup()?;

	let matches = command!()
		.author("Barreleye")
		.version(env!("CARGO_PKG_VERSION"))
		.propagate_version(true)
		.arg(arg!(-e --env <ENV> "Network types to connect to").value_parser(value_parser!(Env)))
		.arg(
			arg!(-c <CONFIG_PATH> "Custom configuration file path")
				.long("config-path")
				.id("config-path"),
		)
		.arg(arg!(--indexer "Run only indexer, without the server"))
		.arg(arg!(--server "Run only server, without the indexer"))
		.arg(arg!(-v --verbose "Verbose mode"))
		.arg(arg!(--bannerless "Hide ASCII banner"))
		.get_matches();

	let env = *matches.get_one("env").unwrap_or(&Env::Mainnet);
	let config_path = matches.get_one::<String>("config-path").map(|s| s.to_string());
	let skip_ascii = *matches.get_one("bannerless").unwrap_or(&false);
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

	let progress = Progress::new(is_indexer);
	progress.show(ProgressStep::Setup);

	let settings = Arc::new(Settings::new(config_path)?);

	let cache = Arc::new(RwLock::new(
		Cache::new(settings.clone())
			.await
			.map_err(|url| {
				quit(AppError::ServiceConnection {
					service: settings.cache.driver.to_string(),
					url: url.to_string(),
				});
			})
			.unwrap(),
	));

	let warehouse = Arc::new(
		Warehouse::new(settings.clone())
			.await
			.map_err(|url| {
				quit(AppError::ServiceConnection {
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
				quit(AppError::ServiceConnection {
					service: settings.db.driver.to_string(),
					url: url.to_string(),
				});
			})
			.unwrap(),
	);

	progress.show(ProgressStep::Migrations);
	warehouse.run_migrations().await?;
	db.run_migrations().await?;

	let app = Arc::new(
		App::new(settings, cache, db, warehouse, env, verbosity, is_indexer, is_server).await?,
	);

	let warnings = app.get_warnings().await?;

	let mut set = JoinSet::new();
	set.spawn(async {
		signal::ctrl_c().await.ok();
		println!("\nSIGINT received; bye 👋");
		Ok(())
	});

	if is_indexer {
		progress.show(ProgressStep::Networks);
		if let Err(e) = app.connect_networks(false).await {
			quit(AppError::NetworkFailure { error: e.to_string() });
		}

		set.spawn({
			let a = app.clone();
			let w = warnings.clone();
			let p = progress.clone();

			async move {
				let indexer = Indexer::new(a);
				indexer.start(w, p).await
			}
		});
	}

	if is_server {
		set.spawn({
			let a = app.clone();
			let w = warnings.clone();
			let p = progress.clone();

			async move {
				let server = Server::new(a);
				server.start(w, p).await
			}
		});
	} else {
		app.set_is_ready();
	}

	while let Some(res) = set.join_next().await {
		let _ = res?;
	}

	Ok(())
}
