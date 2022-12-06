use console::style;
use eyre::{ErrReport, Result};
use futures::future::join_all;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use itertools::{Either, Itertools};
use serde_json::{from_value as json_parse, json};
use std::{
	collections::HashMap,
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
};
use tokio::{
	sync::{mpsc, oneshot, oneshot::Sender, Mutex},
	time::{sleep, Duration},
};

use crate::{Bitcoin, CanExit, ChainModuleId, ChainTrait, Evm, IndexResults};
use barreleye_common::{
	models::{Config, ConfigKey, Network, PrimaryId},
	progress,
	progress::Step,
	utils, AppError, AppState, Blockchain,
};

type BoxedChain = Arc<Box<dyn ChainTrait>>;

#[derive(Clone)]
struct NetworkParams {
	pub config_key: ConfigKey,
	pub range: (u64, Option<u64>),
	pub modules: Vec<ChainModuleId>,
}

impl NetworkParams {
	pub fn new(
		config_key: ConfigKey,
		min: u64,
		max: Option<u64>,
		modules: &[ChainModuleId],
	) -> Self {
		Self { config_key, range: (min, max), modules: modules.to_vec() }
	}
}

#[derive(Clone)]
pub struct Networks {
	app_state: Arc<AppState>,
	networks_map: Arc<Mutex<HashMap<PrimaryId, BoxedChain>>>,
}

impl Networks {
	pub fn new(app_state: Arc<AppState>) -> Self {
		Self { app_state, networks_map: Arc::new(Mutex::new(HashMap::new())) }
	}

	pub async fn connect(mut self) -> Result<Self> {
		progress::show(Step::Networks).await;

		let spinner_style =
			ProgressStyle::with_template("       {spinner}  ↳ {prefix:.bold.dim}: {wide_msg}")
				.unwrap()
				.tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ");

		let m = MultiProgress::new();
		let threads = Network::get_all_by_env(&self.app_state.db, self.app_state.env)
			.await?
			.into_iter()
			.map(|n| {
				let pb = m.add(ProgressBar::new(1_000_000));
				pb.set_style(spinner_style.clone());
				pb.set_prefix(n.name.clone());
				pb.enable_steady_tick(Duration::from_millis(50));

				tokio::spawn({
					let app_state = self.app_state.clone();
					async move {
						let boxed_chain: Box<dyn ChainTrait> = match n.blockchain {
							Blockchain::Bitcoin => {
								Box::new(Bitcoin::new(app_state, n, Some(&pb)).await?)
							}
							Blockchain::Evm => Box::new(Evm::new(app_state, n, Some(&pb)).await?),
						};

						if let Some(rpc) = boxed_chain.get_rpc() {
							pb.finish_with_message(format!(
								"connected to {}",
								utils::with_masked_auth(&rpc)
							));
						} else {
							pb.finish_with_message("could not connect");
						}

						Ok::<_, ErrReport>(boxed_chain)
					}
				})
			})
			.collect::<Vec<_>>();

		let (networks_map, failures): (HashMap<_, _>, Vec<_>) =
			join_all(threads).await.into_iter().partition_map(|result| match result.unwrap() {
				Ok(boxed_chain) => {
					let network_id = boxed_chain.get_network().network_id;
					Either::Left((network_id, Arc::new(boxed_chain)))
				}
				Err(e) => Either::Right(e),
			});

		if !failures.is_empty() {
			progress::quit(AppError::NetworkFailure {
				error: failures.iter().map(|e| format!("- {}", e)).join("\n"),
			});
		}

		self.networks_map = Arc::new(Mutex::new(networks_map));

		Ok(self)
	}

	pub async fn sync_networks(&mut self) -> Result<()> {
		let all_networks = Network::get_all_by_env(&self.app_state.db, self.app_state.env).await?;

		let mut networks_map = self.networks_map.lock().await;

		// drop removed networks
		let all_networks_ids: Vec<PrimaryId> = all_networks.iter().map(|n| n.network_id).collect();
		networks_map.retain(|network_id, _| all_networks_ids.contains(network_id));

		// add new networks
		for n in all_networks
			.into_iter()
			.filter(|n| !networks_map.contains_key(&n.network_id))
			.collect::<Vec<Network>>()
			.into_iter()
		{
			let app_state = self.app_state.clone();
			networks_map.insert(
				n.network_id,
				Arc::new(match n.blockchain {
					Blockchain::Bitcoin => Box::new(Bitcoin::new(app_state, n, None).await?),
					Blockchain::Evm => Box::new(Evm::new(app_state, n, None).await?),
				}),
			);
		}

		Ok(())
	}

	pub async fn index(&mut self) -> Result<()> {
		let mut last_sync_at = utils::now();
		// let mut last_read_block_map = HashMap::<i64, u64>::new();
		let mut index_results = IndexResults::new();
		let mut config_key_map = HashMap::<ConfigKey, serde_json::Value>::new();

		'indexing: loop {
			if !self.app_state.is_leading() {
				sleep(Duration::from_secs(1)).await;
				continue;
			}

			if utils::ago_in_seconds(5) > last_sync_at {
				last_sync_at = utils::now();
				self.sync_networks().await?;
			}

			let networks_map = self.networks_map.lock().await.clone();
			if !networks_map.is_empty() {
				let mut network_params_map = HashMap::new();

				for (network_id, chain) in networks_map.iter() {
					let nid = *network_id as u64;

					// push all modules to retrieve latest blocks
					let last_read_block = {
						let config_key = ConfigKey::IndexerLatestBlock(nid);
						match config_key_map.contains_key(&config_key) {
							true => json_parse::<u64>(config_key_map[&config_key].clone())?,
							_ => {
								let index_latest_block =
									Config::get::<u64>(&self.app_state.db, config_key);

								if let Some(hit) = index_latest_block.await? {
									hit.value
								} else {
									chain.get_last_processed_block().await?
								}
							}
						}
					};
					let block_height = {
						let config_key = ConfigKey::BlockHeight(nid);
						match Config::get::<u64>(&self.app_state.db, config_key).await? {
							Some(hit) if hit.value > last_read_block => hit.value,
							_ => {
								let block_height = chain.get_block_height().await?;

								Config::set::<u64>(&self.app_state.db, config_key, block_height)
									.await?;

								block_height
							}
						}
					};
					if last_read_block < block_height {
						network_params_map.insert(
							(*network_id, None),
							NetworkParams::new(
								ConfigKey::IndexerLatestBlock(nid),
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
							let ck_block_range = ConfigKey::IndexerSyncBlocks(nid, mid);
							let block_range = {
								match config_key_map.contains_key(&ck_block_range) {
									true => json_parse::<(u64, u64)>(
										config_key_map[&ck_block_range].clone(),
									)?,
									_ => {
										match Config::get::<(u64, u64)>(
											&self.app_state.db,
											ck_block_range,
										)
										.await?
										{
											Some(v) => v.value,
											_ => {
												let block_range = (0, last_read_block);

												Config::set::<(u64, u64)>(
													&self.app_state.db,
													ck_block_range,
													block_range,
												)
												.await?;

												block_range
											}
										}
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
						let chain = networks_map[&network_id].clone();
						let should_keep_going = should_keep_going.clone();
						let can_exit = CanExit::new(network_id, module_id, done.clone(), receipt);

						async move {
							let block_range_min = network_params.range.0;
							let block_range_max = network_params.range.1;

							let (block_height, index_results) = chain
								.process_blocks(
									block_range_min,
									block_range_max,
									network_params.modules,
									should_keep_going,
									can_exit,
								)
								.await?;

							println!(
								"{}: {} {} @ {} {}",
								style("Indexing").cyan().bold(),
								style(match block_range_max {
									None => "↗".to_string(),
									_ => "↺".to_string(),
								})
								.bold(),
								style(chain.get_network().name).yellow(),
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

							let config_key = network_params.config_key;
							let config_value = match config_key {
								ConfigKey::IndexerLatestBlock(_) => json!(block_height),
								ConfigKey::IndexerSyncBlocks(_, _) if block_range_max.is_some() => {
									json!((block_height, block_range_max.unwrap()))
								}
								_ => json!(()),
							};

							Ok::<_, ErrReport>((config_key, config_value, index_results))
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

				// pile up `index_results` and update block markers
				for (config_key, config_value, new_data) in results.into_iter() {
					index_results += new_data;
					config_key_map.insert(config_key, config_value);
				}

				// batch save in warehouse
				if index_results.is_ready_to_commit() {
					// push to warehouse
					index_results.commit(&self.app_state.warehouse).await?;

					// commit config marker updates
					for (config_key, config_value) in config_key_map.iter() {
						let db = &self.app_state.db;
						let key = *config_key;

						match config_key {
							ConfigKey::IndexerLatestBlock(_) => {
								let value = json_parse::<u64>(config_value.clone())?;
								Config::set::<u64>(db, key, value).await?;
							}
							ConfigKey::IndexerSyncBlocks(_, _) => {
								let value = json_parse::<(u64, u64)>(config_value.clone())?;
								Config::set::<(u64, u64)>(db, key, value).await?;
							}
							ConfigKey::IndexerSynced(_, _) => {
								let value = json_parse::<u8>(config_value.clone())?;
								Config::set::<u8>(db, key, value).await?;
							}
							_ => {}
						}
					}

					// @TODO if IndexerSynced is preset, delete the associated range (it's not
					// needed anymore)

					config_key_map.clear();
				}
			}
		}
	}
}
