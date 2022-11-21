use console::style;
use eyre::{ErrReport, Result};
use futures::future::join_all;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use itertools::{Either, Itertools};
use std::{
	collections::HashMap,
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
};
use tokio::{
	sync::{mpsc, mpsc::Sender},
	time::{sleep, Duration},
};

use crate::{Bitcoin, ChainTrait, Evm};
use barreleye_common::{
	models::{Config, ConfigKey, Network, PrimaryId, Transfer},
	progress,
	progress::Step,
	utils, AppError, AppState, Blockchain,
};

pub struct Networks {
	app_state: Arc<AppState>,
	networks_map: HashMap<PrimaryId, Arc<Box<dyn ChainTrait>>>,
}

impl Networks {
	pub fn new(app_state: Arc<AppState>) -> Self {
		Self { app_state, networks_map: HashMap::new() }
	}

	pub async fn connect(mut self) -> Result<Self> {
		progress::show(Step::Networks).await;

		let spinner_style = ProgressStyle::with_template(
			"       {spinner}  ↳ {prefix:.bold.dim}: {wide_msg}",
		)
		.unwrap()
		.tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ");

		let m = MultiProgress::new();
		let threads =
			Network::get_all_by_env(&self.app_state.db, self.app_state.env)
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
							let boxed_chain: Box<dyn ChainTrait> = match n
								.blockchain
							{
								Blockchain::Bitcoin => Box::new(
									Bitcoin::new(app_state, n, Some(&pb))
										.await?,
								),
								Blockchain::Evm => Box::new(
									Evm::new(app_state, n, Some(&pb)).await?,
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

							Ok::<_, ErrReport>(boxed_chain)
						}
					})
				})
				.collect::<Vec<_>>();

		let (networks_map, failures): (HashMap<_, _>, Vec<_>) =
			join_all(threads).await.into_iter().partition_map(|result| {
				match result.unwrap() {
					Ok(boxed_chain) => {
						let network_id = boxed_chain.get_network().network_id;
						Either::Left((network_id, Arc::new(boxed_chain)))
					}
					Err(e) => Either::Right(e),
				}
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
		let all_networks =
			Network::get_all_by_env(&self.app_state.db, self.app_state.env)
				.await?;

		// drop removed networks
		let all_networks_ids: Vec<PrimaryId> =
			all_networks.iter().map(|n| n.network_id).collect();
		self.networks_map
			.retain(|network_id, _| all_networks_ids.contains(network_id));

		// add new networks
		for n in all_networks
			.into_iter()
			.filter(|n| !self.networks_map.contains_key(&n.network_id))
			.collect::<Vec<Network>>()
			.into_iter()
		{
			let app_state = self.app_state.clone();
			self.networks_map.insert(
				n.network_id,
				Arc::new(match n.blockchain {
					Blockchain::Bitcoin => {
						Box::new(Bitcoin::new(app_state, n, None).await?)
					}
					Blockchain::Evm => {
						Box::new(Evm::new(app_state, n, None).await?)
					}
				}),
			);
		}

		Ok(())
	}

	pub async fn watch(&mut self) -> Result<()> {
		let mut last_sync_at = utils::now();
		let mut last_save_at = utils::now();
		let mut last_read_block_map = HashMap::<i64, u64>::new();
		let mut transfers = vec![];

		'watching: loop {
			let is_leading =
				self.app_state.is_ready() && self.app_state.is_leader();

			if is_leading && utils::ago_in_seconds(5) > last_sync_at {
				last_sync_at = utils::now();
				self.sync_networks().await?;
			}

			if is_leading && !self.networks_map.is_empty() {
				if utils::ago_in_seconds(5) > last_sync_at {
					last_sync_at = utils::now();
					self.sync_networks().await?;
				}

				let mut network_ids = HashMap::new();
				for (network_id, chain) in self.networks_map.iter() {
					let last_read_block = {
						match last_read_block_map.contains_key(network_id) {
							true => last_read_block_map[network_id],
							_ => {
								match Config::get::<u64>(
									&self.app_state.db,
									ConfigKey::LastSavedBlock(
										*network_id as u64,
									),
								)
								.await?
								{
									Some(hit) => hit.value,
									_ => {
										chain.get_last_processed_block().await?
									}
								}
							}
						}
					};

					let block_height = {
						let config_key =
							ConfigKey::BlockHeight(*network_id as u64);

						match Config::get::<u64>(
							&self.app_state.db,
							config_key.clone(),
						)
						.await?
						{
							Some(hit) if hit.value > last_read_block => {
								hit.value
							}
							_ => {
								let block_height =
									chain.get_block_height().await?;

								Config::set::<u64>(
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
						network_ids.insert(network_id, last_read_block);
					}
				}

				// let the other branch in `tokio::select` below finish first
				let (i_am_done, mut is_done) =
					mpsc::channel(network_ids.len() + 1);

				let mut futures = vec![];
				let should_keep_going = Arc::new(AtomicBool::new(true));
				let mut receipts = HashMap::<PrimaryId, Sender<()>>::new();

				for (network_id, last_read_block) in
					network_ids.clone().into_iter()
				{
					let (rtx, receipt) = mpsc::channel(1);
					receipts.insert(*network_id, rtx);

					futures.push(tokio::spawn({
						let network_id = *network_id;
						let chain = self.networks_map[&network_id].clone();
						let i_am_done = i_am_done.clone();
						let should_keep_going = should_keep_going.clone();

						async move {
							let (block_height, transfers) = chain
								.process_blocks(
									last_read_block,
									should_keep_going,
									i_am_done,
									receipt,
								)
								.await?;

							println!(
								"{} @ block {}…",
								style(chain.get_network().name).yellow(),
								style(block_height).bold()
							);

							Ok::<_, ErrReport>((
								network_id,
								block_height,
								transfers,
							))
						}
					}));
				}

				let results = tokio::select! {
					_ = async {
						while let Some(network_id) = is_done.recv().await {
							network_ids.remove(&network_id);
							if network_ids.is_empty() {
								should_keep_going.store(false, Ordering::SeqCst);
							}

							receipts[&network_id].send(()).await?;
						}

						Ok::<_, ErrReport>(())
					} => Ok(vec![]),
					v = join_all(futures) => {
						let mut out = vec![];

						for result in v.into_iter() {
							match result {
								Ok(v) => out.push(v?),
								Err(e) => {
									return Err(e.into());
								}
							}
						}

						Ok(out)
					},
				};

				// if any of the chains complained, return err
				if results.is_err() {
					break 'watching results.map(|_| ());
				}

				// pile up `transfers` and inc block markers
				for (network_id, last_read_block, new_transfers) in
					results?.into_iter()
				{
					transfers.extend(new_transfers);
					last_read_block_map.insert(network_id, last_read_block);
				}

				// batch save in warehouse
				if utils::ago_in_seconds(5) > last_save_at &&
					transfers.len() > 1_000
				{
					// insert new transfers
					Transfer::create_many(
						&self.app_state.warehouse,
						transfers.clone(),
					)
					.await?;
					transfers.clear();

					// commit latest saved blocks
					Config::set_many::<u64>(
						&self.app_state.db,
						last_read_block_map
							.iter()
							.map(|(network_id, last_read_block)| {
								(
									ConfigKey::LastSavedBlock(
										(*network_id) as u64,
									),
									*last_read_block,
								)
							})
							.collect::<HashMap<ConfigKey, u64>>(),
					)
					.await?;
					last_read_block_map.clear();

					// update timestamp
					last_save_at = utils::now();
				}
			} else {
				sleep(Duration::from_secs(1)).await;
			}
		}
	}
}
