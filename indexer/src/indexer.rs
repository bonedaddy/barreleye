use console::style;
use eyre::{ErrReport, Result};
use num_format::{SystemLocale, ToFormattedString};
use serde_json::{from_value as json_parse, json};
use std::{
	collections::{HashMap, HashSet},
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
};
use tokio::{
	signal,
	sync::{broadcast, mpsc, mpsc::Sender},
	task::JoinSet,
	time::{sleep, Duration},
};
use uuid::Uuid;

use crate::{Lists, Pipe};
use barreleye_common::{
	chain::WarehouseData,
	models::{Config, ConfigKey, PrimaryId},
	quit, utils, App, AppError, BlockHeight, ChainModuleId, Progress, ProgressReadyType,
	ProgressStep, Verbosity, Warnings,
};

#[derive(Clone, Debug)]
struct NetworkParams {
	pub network_id: PrimaryId,
	pub range: (BlockHeight, Option<BlockHeight>),
	pub modules: Vec<ChainModuleId>,
}

impl NetworkParams {
	pub fn new(
		network_id: PrimaryId,
		min: BlockHeight,
		max: Option<BlockHeight>,
		modules: &[ChainModuleId],
	) -> Self {
		Self { network_id, range: (min, max), modules: modules.to_vec() }
	}
}

pub struct Indexer {
	app: Arc<App>,
}

impl Indexer {
	pub fn new(app: Arc<App>) -> Self {
		Self { app }
	}

	pub async fn start(&self, warnings: Warnings, progress: Progress) -> Result<()> {
		if self.app.is_indexer && !self.app.is_server {
			progress.show(ProgressStep::Ready(ProgressReadyType::Indexer, warnings));
		}

		let lists = Lists::new(self.app.clone());

		tokio::select! {
			_ = signal::ctrl_c() => Ok(()),
			v = lists.start_watching() => v,
			v = self.start_primary_check() => v,
			v = self.start_indexing() => {
				if v.is_err() {
					quit(AppError::IndexingFailed {
						error: v.as_ref().unwrap_err().to_string(),
					});
				}

				v
			}
		}
	}

	async fn start_indexing(&self) -> Result<()> {
		let mut warehouse_data = WarehouseData::new();
		let mut config_key_map = HashMap::<ConfigKey, serde_json::Value>::new();
		let verbose = self.app.verbosity as u8 > Verbosity::Silent as u8;
		let mut started_indexing = false;

		let log = |s: &str| println!("{}: {s}", style("Indexer").cyan().bold());
		let num = |n: usize| -> Result<String> {
			let locale = SystemLocale::default()?;
			Ok(n.to_formatted_string(&locale))
		};

		'indexing: loop {
			if !self.app.is_leading() {
				sleep(Duration::from_secs(1)).await;
				continue;
			}

			if !started_indexing {
				started_indexing = true;
				log("Started…");
			}

			self.app.connect_networks(true).await?;

			if !self.app.networks.read().await.is_empty() {
				let mut network_params_map = HashMap::new();

				for (network_id, chain) in self.app.networks.read().await.iter() {
					let nid = *network_id;

					let mut last_read_block =
						Config::get::<BlockHeight>(&self.app.db, ConfigKey::IndexerTailSync(nid))
							.await?
							.map(|h| h.value)
							.unwrap_or(0);

					let block_height = {
						let config_key = ConfigKey::BlockHeight(nid);
						match Config::get::<BlockHeight>(&self.app.db, config_key).await? {
							Some(hit) if hit.value > last_read_block => hit.value,
							_ => {
								let block_height = chain.get_block_height().await?;

								Config::set::<BlockHeight>(&self.app.db, config_key, block_height)
									.await?;

								block_height
							}
						}
					};

					// if first time, split up network into chunks for faster initial syncing
					let chunks = num_cpus::get();
					if last_read_block == 0 &&
						chunks > 0 && Config::get_many_by_keyword::<(BlockHeight, BlockHeight)>(
						&self.app.db,
						&format!("chunk_sync_n{nid}"),
					)
					.await?
					.is_empty()
					{
						let chunk_size = ((block_height - 1) as f64 / chunks as f64).floor() as u64;

						// create chunks
						let block_sync_ranges = {
							let mut ret = HashMap::new();

							let mut block_height_min = 0;
							let mut block_height_max = chunk_size;

							for i in 0..chunks {
								if i + 1 == chunks {
									block_height_max = block_height - 1
								}

								ret.insert(
									ConfigKey::IndexerChunkSync(nid, block_height_max),
									(block_height_min, block_height_max),
								);

								block_height_min = block_height_max + 1;
								block_height_max += chunk_size;
							}

							ret
						};

						// create tail-sync indexes
						Config::set_many::<(BlockHeight, BlockHeight)>(
							&self.app.db,
							block_sync_ranges,
						)
						.await?;

						// fast-forward last read block to almost block_height
						last_read_block = block_height - 1;
						Config::set::<BlockHeight>(
							&self.app.db,
							ConfigKey::IndexerTailSync(nid),
							last_read_block,
						)
						.await?;

						// no need for individual module syncs, so mark all as done
						Config::set_many::<u8>(
							&self.app.db,
							chain
								.get_module_ids()
								.into_iter()
								.map(|module_id| {
									let mid = module_id as u16;
									(ConfigKey::IndexerModuleSynced(nid, mid), 1u8)
								})
								.collect::<HashMap<_, _>>(),
						)
						.await?;
					}

					// push tail index to process latest blocks (incl all modules)
					network_params_map.insert(
						ConfigKey::IndexerTailSync(nid),
						NetworkParams::new(nid, last_read_block, None, &chain.get_module_ids()),
					);

					// push all fast-sync block ranges
					for (config_key, block_range) in Config::get_many_by_keyword::<(
						BlockHeight,
						BlockHeight,
					)>(&self.app.db, &format!("chunk_sync_n{nid}"))
					.await?
					{
						network_params_map.insert(
							config_key,
							NetworkParams::new(
								nid,
								block_range.value.0,
								Some(block_range.value.1),
								&chain.get_module_ids(),
							),
						);
					}

					// push individual modules that need to sync up
					for module_id in chain.get_module_ids().into_iter() {
						let mid = module_id as u16;

						let ck_synced = ConfigKey::IndexerModuleSynced(nid, mid);
						if Config::get::<u8>(&self.app.db, ck_synced).await?.is_none() {
							let ck_block_range = ConfigKey::IndexerModuleSync(nid, mid);

							let block_range = match Config::get::<(BlockHeight, BlockHeight)>(
								&self.app.db,
								ck_block_range,
							)
							.await?
							{
								Some(hit) => hit.value,
								_ => {
									let block_range = (0, last_read_block);

									if last_read_block > 0 {
										Config::set::<(BlockHeight, BlockHeight)>(
											&self.app.db,
											ck_block_range,
											block_range,
										)
										.await?;
									}

									block_range
								}
							};

							if block_range.0 < block_range.1 {
								network_params_map.insert(
									ck_block_range,
									NetworkParams::new(
										nid,
										block_range.0,
										Some(block_range.1),
										&[module_id],
									),
								);
							}
						}
					}
				}

				let (pipe_sender, mut pipe_receiver) = mpsc::channel(network_params_map.len());
				let (abort_sender, _) = broadcast::channel(network_params_map.len());
				let should_keep_going = Arc::new(AtomicBool::new(true));
				let mut receipts = HashMap::<ConfigKey, Sender<()>>::new();

				let thread_count = network_params_map.len();
				if verbose {
					log(&format!("Launching {} thread(s)", style(num(thread_count)?).bold(),));
				}

				let mut futures = JoinSet::new();
				for (config_key, network_params) in network_params_map.clone().into_iter() {
					let (rtx, receipt) = mpsc::channel(1);
					receipts.insert(config_key, rtx);

					futures.spawn({
						let nid = network_params.network_id;
						let networks = self.app.networks.read().await;
						let chain = networks[&network_params.network_id].clone();
						let should_keep_going = should_keep_going.clone();
						let mut pipe = Pipe::new(
							config_key,
							pipe_sender.clone(),
							receipt,
							abort_sender.subscribe(),
						);
						let db = self.app.db.clone();

						async move {
							let mut warehouse_data = WarehouseData::new();

							let mut block_height = network_params.range.0;
							let block_height_max = network_params.range.1;

							let config_value = |block_height| match config_key {
								ConfigKey::IndexerTailSync(_) => json!(block_height),
								ConfigKey::IndexerChunkSync(_, _) |
								ConfigKey::IndexerModuleSync(_, _)
									if block_height_max.is_some() =>
								{
									json!((block_height, block_height_max.unwrap()))
								}
								_ => panic!("no return value for {config_key}"),
							};

							while should_keep_going.load(Ordering::SeqCst) {
								match block_height_max {
									Some(block_height_max)
										if block_height + 1 > block_height_max =>
									{
										if !warehouse_data.is_empty() {
											pipe.push(
												config_value(block_height),
												warehouse_data.clone(),
											)
											.await?;
										}

										break;
									}
									None => {
										let config_key = ConfigKey::BlockHeight(nid);
										let saved_block_height =
											Config::get::<BlockHeight>(&db, config_key)
												.await?
												.map(|v| v.value)
												.unwrap_or(0);

										if block_height + 1 > saved_block_height {
											let latest_block_height =
												chain.get_block_height().await?;

											if latest_block_height > saved_block_height {
												Config::set::<BlockHeight>(
													&db,
													config_key,
													latest_block_height,
												)
												.await?;
											} else {
												let timeout = chain.get_network().block_time_ms;
												sleep(Duration::from_millis(timeout as u64)).await;
												continue;
											}
										}
									}
									_ => {}
								}

								block_height += 1;

								let is_done = tokio::select! {
									_ = pipe.abort.recv() => true,
									new_data = chain.process_block(
										block_height,
										network_params.modules.clone(),
									) => match new_data? {
										Some(new_data) => {
											warehouse_data += new_data;
											false
										},
										None => true,
									},
								};

								if is_done || warehouse_data.len() > 100 {
									pipe.push(config_value(block_height), warehouse_data.clone())
										.await?;
									warehouse_data.clear();
								}

								if is_done {
									break;
								}
							}

							Ok::<_, ErrReport>(())
						}
					});
				}

				// drop the original non-cloned
				drop(pipe_sender);

				// vars to keep network in sync
				let networks_updated_at =
					Config::get::<u8>(&self.app.db, ConfigKey::NetworksUpdated)
						.await?
						.map(|v| v.updated_at)
						.unwrap_or_else(utils::now);

				let abort = || -> Result<()> {
					should_keep_going.store(false, Ordering::SeqCst);
					abort_sender.send(())?;
					Ok(())
				};

				// process thread returns + their outputs
				loop {
					tokio::select! {
						_ = sleep(Duration::from_secs(1)) => {
							if let Some(value) =
								Config::get::<u8>(&self.app.db, ConfigKey::NetworksUpdated)
									.await?
							{
								if value.updated_at != networks_updated_at {
									if verbose {
										log("Restarting… (networks updated)");
									}

									abort()?;
									break;
								}
							}
						}
						result = futures.join_next() => {
							if let Some(task_result) = result {
								if let Err(e) = task_result? {
									break 'indexing Err(e);
								}
							} else {
								break;
							}
						}
						Some((config_key, config_value, new_data)) = pipe_receiver.recv() => {
							if !self.app.is_leading() {
								abort()?;
								break;
							}

							if verbose {
								log(&format!(
									"Thread {} returned {} record(s)",
									style(config_key).bold(),
									num(new_data.len())?,
								));
							}

							// update results
							warehouse_data += new_data;
							config_key_map.insert(config_key, config_value);

							// release thread so it can keep going
							if let Some(receipt) = receipts.get(&config_key) {
								receipt.send(()).await.unwrap();
							}

							// batch save in warehouse
							if warehouse_data.should_commit() {
								let mut updated_network_ids = HashSet::new();

								if verbose {
									log(&format!(
										"Pushing {} record(s) to warehouse",
										style(num(warehouse_data.len())?).bold(),
									));
								}

								// push to warehouse
								warehouse_data.commit(&self.app.warehouse).await?;

								// commit config marker updates
								for (config_key, config_value) in config_key_map.iter() {
									let db = &self.app.db;
									let key = *config_key;
									let value = config_value.clone();

									match config_key {
										ConfigKey::IndexerTailSync(nid) => {
											let value = json_parse::<BlockHeight>(value)?;
											Config::set::<BlockHeight>(db, key, value).await?;

											updated_network_ids.insert(*nid);
										}
										ConfigKey::IndexerChunkSync(nid, _) => {
											let (block_range_min, block_range_max) =
												json_parse::<(BlockHeight, BlockHeight)>(value)?;

											if block_range_min < block_range_max {
												Config::set::<(BlockHeight, BlockHeight)>(
													db,
													key,
													(block_range_min, block_range_max),
												)
												.await?;
											} else {
												Config::delete(db, key).await?;
											}

											updated_network_ids.insert(*nid);
										}
										ConfigKey::IndexerModuleSync(nid, mid) => {
											let value =
												json_parse::<(BlockHeight, BlockHeight)>(value)?;
											Config::set::<(BlockHeight, BlockHeight)>(db, key, value)
												.await?;

											if value.0 >= value.1 {
												Config::set::<u8>(
													db,
													ConfigKey::IndexerModuleSynced(*nid, *mid),
													1,
												)
												.await?;
											}

											updated_network_ids.insert(*nid);
										}
										ConfigKey::IndexerModuleSynced(nid, _) => {
											let value = json_parse::<u8>(value)?;
											Config::set::<u8>(db, key, value).await?;

											updated_network_ids.insert(*nid);
										}
										_ => {}
									}
								}

								// cleanup: if `config_key_map` contains a key indicating a certain
								// module has been fully synced, it's safe to delete config for its
								// range markers
								for (config_key, _) in config_key_map.iter() {
									if let ConfigKey::IndexerModuleSynced(nid, mid) = config_key {
										let ck_block_range = ConfigKey::IndexerModuleSync(*nid, *mid);
										Config::delete(&self.app.db, ck_block_range).await?;
									}
								}

								// reset config key markers
								config_key_map.clear();

								// update progress for each network
								for network_id in updated_network_ids.into_iter() {
									let nid = network_id;
									let mut scores = vec![];

									let networks = self.app.networks.read().await;
									let chain = networks[&network_id].clone();

									let block_height = Config::get::<BlockHeight>(
										&self.app.db,
										ConfigKey::BlockHeight(nid),
									)
									.await?
									.map(|v| v.value)
									.unwrap_or(0);

									if block_height == 0 {
										scores.push(0.0);
									} else {
										let tail_block = Config::get::<BlockHeight>(
											&self.app.db,
											ConfigKey::IndexerTailSync(nid),
										)
										.await?
										.map(|v| v.value)
										.unwrap_or(0);

										let mut done_blocks = tail_block;
										for (_, block_range) in
											Config::get_many_by_keyword::<(BlockHeight, BlockHeight)>(
												&self.app.db,
												&format!("chunk_sync_n{nid}"),
											)
											.await?
										{
											done_blocks -= block_range.value.1 - block_range.value.0;
										}

										scores.push(done_blocks as f64 / block_height as f64);

										for module_id in chain.get_module_ids().into_iter() {
											let mid = module_id as u16;

											let ck_synced = ConfigKey::IndexerModuleSynced(nid, mid);
											if Config::get::<u8>(&self.app.db, ck_synced)
												.await?
												.is_none()
											{
												let (block_range_min, block_range_max) =
													Config::get::<(BlockHeight, BlockHeight)>(
														&self.app.db,
														ConfigKey::IndexerModuleSync(nid, mid),
													)
													.await?
													.map(|v| v.value)
													.unwrap_or((0, tail_block));

												if block_range_max > block_range_min {
													let indexed = done_blocks -
														(block_range_max - block_range_min);
													scores.push(indexed as f64 / block_height as f64);
												}
											}
										}
									}

									let progress = scores.iter().sum::<f64>() / scores.len() as f64;
									Config::set::<f64>(
										&self.app.db,
										ConfigKey::IndexerProgress(nid),
										progress,
									)
									.await?;

									log(&format!(
										"{} @ {:.4}%…",
										style(chain.get_network().name).bold(),
										progress * 100.0,
									));
								}
							}
						}
					}
				}
			}
		}
	}

	async fn start_primary_check(&self) -> Result<()> {
		let primary_promotion = self.app.settings.primary_promotion;
		let db = &self.app.db;
		let uuid = self.app.uuid;

		loop {
			let cool_down_period = utils::ago_in_seconds(primary_promotion / 2);

			let last_primary = Config::get::<Uuid>(db, ConfigKey::Primary).await?;
			match last_primary {
				None => {
					// first run ever
					Config::set::<Uuid>(db, ConfigKey::Primary, uuid).await?;
				}
				Some(hit) if hit.value == uuid && hit.updated_at >= cool_down_period => {
					// if primary, check-in only if cool-down period has not started yet ↑
					if Config::set_where::<Uuid>(db, ConfigKey::Primary, uuid, hit).await? {
						self.app.set_is_primary(true).await?;
					}
				}
				Some(hit) if utils::ago_in_seconds(primary_promotion) > hit.updated_at => {
					// attempt to upgrade to primary (set is_primary on the next iteration)
					Config::set_where::<Uuid>(db, ConfigKey::Primary, uuid, hit).await?;
				}
				_ => {
					// either cool-down period has started or this is a secondary
					self.app.set_is_primary(false).await?;
				}
			}

			sleep(Duration::from_secs(self.app.settings.primary_ping)).await
		}
	}
}
