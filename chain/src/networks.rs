use console::style;
use eyre::{ErrReport, Result};
use futures::future::join_all;
use governor::{Quota, RateLimiter as GovernorRateLimiter};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use itertools::{Either, Itertools};
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
	sync::{mpsc, oneshot, oneshot::Sender},
	time::{sleep, Duration},
};

use crate::{Bitcoin, CanExit, ChainModuleId, ChainTrait, Evm, RateLimiter, WarehouseData};
use barreleye_common::{
	models::{Config, ConfigKey, Network, PrimaryId},
	progress,
	progress::Step,
	utils, AppError, AppState, BlockHeight, Blockchain, Verbosity,
};

#[derive(Clone)]
struct NetworkParams {
	pub config_key: ConfigKey,
	pub range: (BlockHeight, Option<BlockHeight>),
	pub modules: Vec<ChainModuleId>,
}

impl NetworkParams {
	pub fn new(
		config_key: ConfigKey,
		min: BlockHeight,
		max: Option<BlockHeight>,
		modules: &[ChainModuleId],
	) -> Self {
		Self { config_key, range: (min, max), modules: modules.to_vec() }
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
			.map(|network| {
				let pb = m.add(ProgressBar::new(1_000_000));
				pb.set_style(spinner_style.clone());
				pb.set_prefix(network.name.clone());
				pb.enable_steady_tick(Duration::from_millis(50));

				tokio::spawn({
					let app_state = self.app_state.clone();
					let rate_limiter = self.get_rate_limiter(network.rps as u32);

					async move {
						let boxed_chain: Box<dyn ChainTrait> = match network.blockchain {
							Blockchain::Bitcoin => Box::new(
								Bitcoin::new(app_state, network, rate_limiter, Some(&pb)).await?,
							),
							Blockchain::Evm => Box::new(
								Evm::new(app_state, network, rate_limiter, Some(&pb)).await?,
							),
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
			join_all(threads).await.into_iter().partition_map(|result| match result.unwrap() {
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
		let all_networks_ids: Vec<PrimaryId> = all_networks.iter().map(|n| n.network_id).collect();
		self.networks_map.retain(|network_id, _| all_networks_ids.contains(network_id));

		// add new networks
		for network in all_networks
			.into_iter()
			.filter(|n| !self.networks_map.contains_key(&n.network_id))
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

	pub async fn index(&mut self) -> Result<()> {
		let mut last_sync_at = utils::now();
		let mut warehouse_data = WarehouseData::new();
		let mut config_key_map = HashMap::<ConfigKey, serde_json::Value>::new();
		let verbosity = self.app_state.verbosity;

		'indexing: loop {
			if !self.app_state.is_leading() {
				sleep(Duration::from_secs(1)).await;
				continue;
			}

			if utils::ago_in_seconds(5) > last_sync_at {
				last_sync_at = utils::now();
				self.sync_networks().await?;
			}

			if !self.networks_map.is_empty() {
				let mut network_params_map = HashMap::new();

				for (network_id, chain) in self.networks_map.iter() {
					let nid = *network_id as u64;

					// push all modules to retrieve latest blocks
					let last_read_block = {
						let config_key = ConfigKey::IndexerTailBlock(nid);
						match config_key_map.contains_key(&config_key) {
							true => json_parse::<BlockHeight>(config_key_map[&config_key].clone())?,
							_ => match Config::get::<BlockHeight>(&self.app_state.db, config_key)
								.await?
							{
								Some(hit) => hit.value,
								_ => chain.get_last_processed_block().await?,
							},
						}
					};

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

					if last_read_block < block_height {
						network_params_map.insert(
							(*network_id, None),
							NetworkParams::new(
								ConfigKey::IndexerTailBlock(nid),
								last_read_block,
								None,
								&chain.get_module_ids(),
							),
						);
					}

					// push only those modules that have yet to be synced
					for module_id in chain.get_module_ids().into_iter() {
						let mid = module_id as u16;

						let ck_synced = ConfigKey::IndexerSynced(nid, mid);
						if Config::get::<u8>(&self.app_state.db, ck_synced).await?.is_none() {
							let ck_block_range = ConfigKey::IndexerHeadBlocks(nid, mid);
							let block_range = if config_key_map.contains_key(&ck_block_range) {
								json_parse::<(BlockHeight, BlockHeight)>(
									config_key_map[&ck_block_range].clone(),
								)?
							} else {
								match Config::get::<(BlockHeight, BlockHeight)>(
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
								}
							};

							if block_range.0 >= block_range.1 {
								config_key_map.insert(ck_synced, json!(1));
							} else {
								network_params_map.insert(
									(*network_id, Some(module_id)),
									NetworkParams::new(
										ck_block_range,
										block_range.0,
										Some(block_range.1),
										&[module_id],
									),
								);
							}
						}
					}
				}

				let (done, mut is_done) = mpsc::channel(network_params_map.len());
				let mut futures = vec![];
				let should_keep_going = Arc::new(AtomicBool::new(true));
				let mut receipts = HashMap::<PrimaryId, Sender<()>>::new();

				for ((network_id, module_id), network_params) in
					network_params_map.clone().into_iter()
				{
					let (rtx, receipt) = oneshot::channel();
					receipts.insert(network_id, rtx);

					futures.push(tokio::spawn({
						let chain = self.networks_map[&network_id].clone();
						let should_keep_going = should_keep_going.clone();
						let can_exit = CanExit::new(network_id, module_id, done.clone(), receipt);

						async move {
							let block_range_min = network_params.range.0;
							let block_range_max = network_params.range.1;

							let (block_height, warehouse_data) = chain
								.process_blocks(
									block_range_min,
									block_range_max,
									network_params.modules.clone(),
									should_keep_going,
									can_exit,
								)
								.await?;

							if verbosity as u8 > Verbosity::Silent as u8 {
								println!(
									"{}: {} {}{} @ {} {}",
									style("Indexing").cyan().bold(),
									style(match block_range_max {
										None => "↗".to_string(),
										_ => "↺".to_string(),
									})
									.bold(),
									style(chain.get_network().name).bold(),
									match block_range_max {
										None => "".to_string(),
										_ => format!(" ({})", network_params.modules[0]),
									},
									match block_range_max {
										None => "block".to_string(),
										_ => "blocks".to_string(),
									},
									style(match block_range_max {
										None => format!("{block_height}…"),
										_ => format!("{block_range_min}…{block_height}"),
									})
									.bold(),
								);
							}

							let config_key = network_params.config_key;
							let config_value = match config_key {
								ConfigKey::IndexerTailBlock(_) => json!(block_height),
								ConfigKey::IndexerHeadBlocks(_, _) if block_range_max.is_some() => {
									json!((block_height, block_range_max.unwrap()))
								}
								_ => json!(()),
							};

							Ok::<_, ErrReport>((config_key, config_value, warehouse_data))
						}
					}));
				}

				drop(done); // drop the original non-cloned
				while let Some((network_id, module_id)) = is_done.recv().await {
					network_params_map.remove(&(network_id, module_id));

					if network_params_map.is_empty() {
						should_keep_going.store(false, Ordering::SeqCst);
					}

					if let Some(receipt) = receipts.remove(&network_id) {
						receipt.send(()).unwrap();
					}
				}

				let mut results = vec![];
				for future in futures.drain(..) {
					match future.await {
						Ok(v) => match v {
							Ok(result) => results.push(result),
							_ => break 'indexing v.map(|_| ()),
						},
						Err(e) => break 'indexing Err(e.into()),
					}
				}

				// pile up `warehouse_data` and update block markers
				for (config_key, config_value, new_data) in results.into_iter() {
					warehouse_data += new_data;
					config_key_map.insert(config_key, config_value);
				}

				// batch save in warehouse
				if warehouse_data.should_commit() {
					let mut updated_network_ids = HashSet::new();

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
							ConfigKey::IndexerHeadBlocks(nid, _) => {
								let value = json_parse::<(BlockHeight, BlockHeight)>(value)?;
								Config::set::<(BlockHeight, BlockHeight)>(db, key, value).await?;

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

					// if `config_key_map` contains a key indicating a certain module has been
					// fully synced, it's safe to delete config for its range markers
					for (config_key, _) in config_key_map.iter() {
						if let ConfigKey::IndexerSynced(nid, mid) = config_key {
							let ck_block_range = ConfigKey::IndexerHeadBlocks(*nid, *mid);
							Config::delete(&self.app_state.db, ck_block_range).await?;
						}
					}

					config_key_map.clear();

					// update progress for each network
					for network_id in updated_network_ids.into_iter() {
						let nid = network_id;
						let mut scores = vec![];
						let chain = self.networks_map[&(network_id as i64)].clone();

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

							scores.push(tail_block as f64 / block_height as f64);

							for module_id in chain.get_module_ids().into_iter() {
								let mid = module_id as u16;

								let ck_synced = ConfigKey::IndexerSynced(nid, mid);
								if Config::get::<u8>(&self.app_state.db, ck_synced).await?.is_none()
								{
									let block_range = Config::get::<(BlockHeight, BlockHeight)>(
										&self.app_state.db,
										ConfigKey::IndexerHeadBlocks(nid, mid),
									)
									.await?
									.map(|v| v.value)
									.unwrap_or((0, tail_block));

									if block_range.1 > block_range.0 {
										let indexed = block_height -
											(block_height - tail_block) - (block_range.1 -
											block_range.0);
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

						println!(
							"{}: {} @ {:.4}%…",
							style("Indexing").cyan().bold(),
							style(chain.get_network().name).bold(),
							progress * 100.0,
						);
					}
				}
			}
		}
	}
}
