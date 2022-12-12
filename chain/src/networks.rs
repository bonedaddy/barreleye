use console::style;
use eyre::{ErrReport, Result};
use futures::future::join_all;
use governor::{Quota, RateLimiter as GovernorRateLimiter};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use itertools::{Either, Itertools};
use num_format::{SystemLocale, ToFormattedString};
use serde_json::{from_value as json_parse, json};
use std::{
	collections::{HashMap, HashSet},
	num::NonZeroU32,
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
};
use tokio::{
	sync::{mpsc, mpsc::Sender},
	time::{sleep, Duration},
};

use crate::{Bitcoin, ChainModuleId, ChainTrait, Evm, Pipe, RateLimiter, WarehouseData};
use barreleye_common::{
	models::{Config, ConfigKey, Network, PrimaryId},
	progress,
	progress::Step,
	utils, AppError, AppState, BlockHeight, Blockchain, Verbosity,
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

pub struct Networks {
	app_state: Arc<AppState>,
	networks_map: HashMap<PrimaryId, Arc<Box<dyn ChainTrait>>>,
}

impl Networks {
	pub fn new(app_state: Arc<AppState>) -> Self {
		Self { app_state, networks_map: HashMap::new() }
	}

	fn get_rate_limiter(&self, rps: u32) -> Option<Arc<RateLimiter>> {
		NonZeroU32::new(rps).map(|non_zero_rps| {
			Arc::new(GovernorRateLimiter::direct(Quota::per_second(non_zero_rps)))
		})
	}

	pub async fn connect(mut self) -> Result<Self> {
		progress::show(Step::Networks).await;

		let template = "       {spinner}  ↳ {prefix:.bold.dim}: {wide_msg}";
		let spinner_style = ProgressStyle::with_template(template).unwrap().tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ");

		let m = MultiProgress::new();
		let threads = Network::get_all_by_env(&self.app_state.db, self.app_state.env)
			.await?
			.into_iter()
			.filter(|n| n.is_active)
			.map(|n| {
				let pb = m.add(ProgressBar::new(1_000_000));
				pb.set_style(spinner_style.clone());
				pb.set_prefix(n.name.clone());
				pb.enable_steady_tick(Duration::from_millis(50));

				tokio::spawn({
					let app_state = self.app_state.clone();
					let rate_limiter = self.get_rate_limiter(n.rps as u32);

					async move {
						let boxed_chain: Box<dyn ChainTrait> = match n.blockchain {
							Blockchain::Bitcoin => {
								Box::new(Bitcoin::new(app_state, n, rate_limiter, Some(&pb)).await?)
							}
							Blockchain::Evm => {
								Box::new(Evm::new(app_state, n, rate_limiter, Some(&pb)).await?)
							}
						};

						if let Some(rpc) = boxed_chain.get_rpc() {
							pb.finish_with_message(format!(
								"connected to {}",
								utils::with_masked_auth(&rpc)
							));
						} else {
							pb.finish_with_message("could not connect");
						}

						Ok::<_, ErrReport>(Arc::new(boxed_chain))
					}
				})
			})
			.collect::<Vec<_>>();

		let (networks_map, failures): (HashMap<_, _>, Vec<_>) =
			join_all(threads).await.into_iter().partition_map(|r| match r.unwrap() {
				Ok(chain) => {
					let network_id = chain.get_network().network_id;
					Either::Left((network_id, chain))
				}
				Err(e) => Either::Right(e),
			});

		if !failures.is_empty() {
			progress::quit(AppError::NetworkFailure {
				error: failures.iter().map(|e| format!("- {}", e)).join("\n"),
			});
		}

		self.networks_map = networks_map;

		Ok(self)
	}

	pub async fn sync_networks(&mut self) -> Result<()> {
		let all_networks = Network::get_all_by_env(&self.app_state.db, self.app_state.env).await?;

		// drop removed networks
		let all_active_network_ids: Vec<PrimaryId> = all_networks
			.iter()
			.filter_map(|n| match n.is_active {
				true => Some(n.network_id),
				_ => None,
			})
			.collect();
		self.networks_map.retain(|network_id, _| all_active_network_ids.contains(network_id));

		// add new networks
		for network in all_networks
			.into_iter()
			.filter(|n| {
				n.is_active &&
					(!self.networks_map.contains_key(&n.network_id) ||
						self.networks_map[&n.network_id].get_network() != *n)
			})
			.collect::<Vec<Network>>()
			.into_iter()
		{
			let app_state = self.app_state.clone();
			let rate_limiter = self.get_rate_limiter(network.rps as u32);

			self.networks_map.insert(
				network.network_id,
				Arc::new(match network.blockchain {
					Blockchain::Bitcoin => {
						Box::new(Bitcoin::new(app_state, network, rate_limiter, None).await?)
					}
					Blockchain::Evm => {
						Box::new(Evm::new(app_state, network, rate_limiter, None).await?)
					}
				}),
			);
		}

		Ok(())
	}

	pub async fn networks_have_changed(&self) -> Result<bool> {
		let all_active_networks: HashMap<PrimaryId, Network> =
			Network::get_all_by_env(&self.app_state.db, self.app_state.env)
				.await?
				.into_iter()
				.filter_map(|n| match n.is_active {
					true => Some((n.network_id, n)),
					_ => None,
				})
				.collect();

		if all_active_networks.len() != self.networks_map.len() {
			return Ok(true);
		}

		for (network_id, chain) in self.networks_map.iter() {
			match all_active_networks.get(network_id) {
				Some(network) if *network != chain.get_network() => return Ok(true),
				None => return Ok(true),
				_ => {}
			}
		}

		Ok(false)
	}

	pub async fn index(&mut self) -> Result<()> {
		let mut warehouse_data = WarehouseData::new();
		let mut config_key_map = HashMap::<ConfigKey, serde_json::Value>::new();
		let detailed_logging = self.app_state.verbosity as u8 > Verbosity::Silent as u8;
		let mut started_indexing = false;

		let log = |s: &str| println!("{}: {s}", style("Indexer").cyan().bold());
		let num = |n: usize| -> Result<String> {
			let locale = SystemLocale::default()?;
			Ok(n.to_formatted_string(&locale))
		};

		loop {
			if !self.app_state.is_leading() {
				sleep(Duration::from_secs(1)).await;
				continue;
			}

			if !started_indexing {
				started_indexing = true;
				log("Started…");
			}

			self.sync_networks().await?;

			if !self.networks_map.is_empty() {
				let mut network_params_map = HashMap::new();

				for (network_id, chain) in self.networks_map.iter() {
					let nid = *network_id;

					let mut last_read_block = Config::get::<BlockHeight>(
						&self.app_state.db,
						ConfigKey::IndexerTailBlock(nid),
					)
					.await?
					.map(|h| h.value)
					.unwrap_or(0);

					let block_height = {
						let config_key = ConfigKey::BlockHeight(nid);
						match Config::get::<BlockHeight>(&self.app_state.db, config_key).await? {
							Some(hit) if hit.value > last_read_block => hit.value,
							_ => {
								let block_height = chain.get_block_height().await?;

								Config::set::<BlockHeight>(
									&self.app_state.db,
									config_key,
									block_height,
								)
								.await?;

								block_height
							}
						}
					};

					// if first time, split up network into chunks for faster initial syncing
					let chunks = num_cpus::get();
					if last_read_block == 0 &&
						chunks > 0 && Config::get_many_by_keyword::<(BlockHeight, BlockHeight)>(
						&self.app_state.db,
						&format!("tail_sync_n{nid}"),
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
									ConfigKey::IndexerTailSyncBlocks(nid, block_height_max),
									(block_height_min, block_height_max),
								);

								block_height_min = block_height_max + 1;
								block_height_max += chunk_size;
							}

							ret
						};

						// create tail-sync indexes
						Config::set_many::<(BlockHeight, BlockHeight)>(
							&self.app_state.db,
							block_sync_ranges,
						)
						.await?;

						// fast-forward last read block to almost block_height
						last_read_block = block_height - 1;
						Config::set::<BlockHeight>(
							&self.app_state.db,
							ConfigKey::IndexerTailBlock(nid),
							last_read_block,
						)
						.await?;

						// no need for individual module syncs, so mark all as done
						Config::set_many::<u8>(
							&self.app_state.db,
							chain
								.get_module_ids()
								.into_iter()
								.map(|module_id| {
									let mid = module_id as u16;
									(ConfigKey::IndexerSynced(nid, mid), 1u8)
								})
								.collect::<HashMap<_, _>>(),
						)
						.await?;
					}

					// push tail index to process latest blocks (incl all modules)
					network_params_map.insert(
						ConfigKey::IndexerTailBlock(nid),
						NetworkParams::new(nid, last_read_block, None, &chain.get_module_ids()),
					);

					// push all fast-sync block ranges
					for (config_key, block_range) in
						Config::get_many_by_keyword::<(BlockHeight, BlockHeight)>(
							&self.app_state.db,
							&format!("tail_sync_n{nid}"),
						)
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

						let ck_synced = ConfigKey::IndexerSynced(nid, mid);
						if Config::get::<u8>(&self.app_state.db, ck_synced).await?.is_none() {
							let ck_block_range = ConfigKey::IndexerHeadBlocks(nid, mid);

							let block_range = match Config::get::<(BlockHeight, BlockHeight)>(
								&self.app_state.db,
								ck_block_range,
							)
							.await?
							{
								Some(hit) => hit.value,
								_ => {
									let block_range = (0, last_read_block);

									if last_read_block > 0 {
										Config::set::<(BlockHeight, BlockHeight)>(
											&self.app_state.db,
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
				let should_keep_going = Arc::new(AtomicBool::new(true));
				let mut receipts = HashMap::<ConfigKey, Sender<()>>::new();

				let thread_count = network_params_map.len();
				if detailed_logging {
					log(&format!("Launching {} thread(s)", style(num(thread_count)?).bold(),));
				}

				let mut futures = vec![];
				for (config_key, network_params) in network_params_map.clone().into_iter() {
					let (rtx, receipt) = mpsc::channel(1);
					receipts.insert(config_key, rtx);

					futures.push(tokio::spawn({
						let nid = network_params.network_id;
						let chain = self.networks_map[&network_params.network_id].clone();
						let should_keep_going = should_keep_going.clone();
						let mut pipe = Pipe::new(config_key, pipe_sender.clone(), receipt);
						let db = self.app_state.db.clone();

						async move {
							let mut warehouse_data = WarehouseData::new();

							let mut block_height = network_params.range.0;
							let block_height_max = network_params.range.1;

							while should_keep_going.load(Ordering::SeqCst) {
								match block_height_max {
									Some(block_height_max)
										if block_height + 1 > block_height_max =>
									{
										break
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

								let should_stop = chain
									.process_block(block_height, network_params.modules.clone())
									.await?
									.map(|new_data| {
										warehouse_data += new_data;
										false
									})
									.unwrap_or(true);

								if should_stop || warehouse_data.len() > 500 {
									let config_value = match config_key {
										ConfigKey::IndexerTailBlock(_) => json!(block_height),
										ConfigKey::IndexerTailSyncBlocks(_, _) |
										ConfigKey::IndexerHeadBlocks(_, _)
											if block_height_max.is_some() =>
										{
											json!((block_height, block_height_max.unwrap()))
										}
										_ => panic!("no return value for {config_key}"),
									};

									pipe.push(config_value, warehouse_data.clone()).await?;
									warehouse_data.clear();
								}

								if should_stop {
									break;
								}
							}

							Ok::<_, ErrReport>(())
						}
					}));
				}

				// drop the original non-cloned
				drop(pipe_sender);

				// process received warehouse data from threads and periodically
				// check that this indexer should keep going
				loop {
					tokio::select! {
						_ = sleep(Duration::from_secs(5)) => {
							if !self.app_state.is_leading() || self.networks_have_changed().await? {
								should_keep_going.store(false, Ordering::SeqCst);
								break;
							}
						},
						Some((config_key, config_value, new_data)) = pipe_receiver.recv() => {
							if detailed_logging {
								log(&format!(
									"Thread {} produced {} record(s)",
									style(config_key).bold(),
									num(new_data.len())?,
								));
							}

							warehouse_data += new_data;
							config_key_map.insert(config_key, config_value);

							// batch save in warehouse
							if warehouse_data.should_commit() {
								let mut updated_network_ids = HashSet::new();

								if detailed_logging {
									log(&format!(
										"Pushing {} record(s) to warehouse",
										style(num(warehouse_data.len())?).bold(),
									));
								}

								// push to warehouse
								warehouse_data.commit(&self.app_state.warehouse).await?;

								// commit config marker updates
								for (config_key, config_value) in config_key_map.iter() {
									let db = &self.app_state.db;
									let key = *config_key;
									let value = config_value.clone();

									match config_key {
										ConfigKey::IndexerTailBlock(nid) => {
											let value = json_parse::<BlockHeight>(value)?;
											Config::set::<BlockHeight>(db, key, value).await?;

											updated_network_ids.insert(*nid);
										}
										ConfigKey::IndexerTailSyncBlocks(nid, _) => {
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
										ConfigKey::IndexerHeadBlocks(nid, mid) => {
											let value = json_parse::<(BlockHeight, BlockHeight)>(value)?;
											Config::set::<(BlockHeight, BlockHeight)>(db, key, value)
												.await?;

											if value.0 >= value.1 {
												Config::set::<u8>(
													db,
													ConfigKey::IndexerSynced(*nid, *mid),
													1,
												)
												.await?;
											}

											updated_network_ids.insert(*nid);
										}
										ConfigKey::IndexerSynced(nid, _) => {
											let value = json_parse::<u8>(value)?;
											Config::set::<u8>(db, key, value).await?;

											updated_network_ids.insert(*nid);
										}
										_ => {}
									}
								}

								// cleanup: if `config_key_map` contains a key indicating a certain module
								// has been fully synced, it's safe to delete config for its range markers
								for (config_key, _) in config_key_map.iter() {
									if let ConfigKey::IndexerSynced(nid, mid) = config_key {
										let ck_block_range = ConfigKey::IndexerHeadBlocks(*nid, *mid);
										Config::delete(&self.app_state.db, ck_block_range).await?;
									}
								}

								// reset config key markers
								config_key_map.clear();

								// update progress for each network
								for network_id in updated_network_ids.into_iter() {
									let nid = network_id;
									let mut scores = vec![];
									let chain = self.networks_map[&network_id].clone();

									let block_height = Config::get::<BlockHeight>(
										&self.app_state.db,
										ConfigKey::BlockHeight(nid),
									)
									.await?
									.map(|v| v.value)
									.unwrap_or(0);

									if block_height == 0 {
										scores.push(0.0);
									} else {
										let tail_block = Config::get::<BlockHeight>(
											&self.app_state.db,
											ConfigKey::IndexerTailBlock(nid),
										)
										.await?
										.map(|v| v.value)
										.unwrap_or(0);

										let mut done_blocks = tail_block;
										for (_, block_range) in
											Config::get_many_by_keyword::<(BlockHeight, BlockHeight)>(
												&self.app_state.db,
												&format!("tail_sync_n{nid}"),
											)
											.await?
										{
											done_blocks -= block_range.value.1 - block_range.value.0;
										}

										scores.push(done_blocks as f64 / block_height as f64);

										for module_id in chain.get_module_ids().into_iter() {
											let mid = module_id as u16;

											let ck_synced = ConfigKey::IndexerSynced(nid, mid);
											if Config::get::<u8>(&self.app_state.db, ck_synced)
												.await?
												.is_none()
											{
												let (block_range_min, block_range_max) =
													Config::get::<(BlockHeight, BlockHeight)>(
														&self.app_state.db,
														ConfigKey::IndexerHeadBlocks(nid, mid),
													)
													.await?
													.map(|v| v.value)
													.unwrap_or((0, tail_block));

												if block_range_max > block_range_min {
													let indexed =
														done_blocks - (block_range_max - block_range_min);
													scores.push(indexed as f64 / block_height as f64);
												}
											}
										}
									}

									let progress = scores.iter().sum::<f64>() / scores.len() as f64;
									Config::set::<f64>(
										&self.app_state.db,
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

							// release thread so it can keep going
							if let Some(receipt) = receipts.get(&config_key) {
								receipt.send(()).await.unwrap();
							}
						}
					}
				}

				// wait for all futures to finish
				join_all(futures).await;
			}
		}
	}
}
