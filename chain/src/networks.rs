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
	sync::{mpsc, oneshot, oneshot::Sender, Mutex},
	time::{sleep, Duration},
};

use crate::{Bitcoin, CanExit, ChainTrait, Evm, IndexResults};
use barreleye_common::{
	models::{Config, ConfigKey, Network, PrimaryId},
	progress,
	progress::Step,
	utils, AppError, AppState, Blockchain,
};

type BoxedChain = Arc<Box<dyn ChainTrait>>;

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

		self.networks_map = Arc::new(Mutex::new(networks_map));

		Ok(self)
	}

	pub async fn sync_networks(&mut self) -> Result<()> {
		let all_networks =
			Network::get_all_by_env(&self.app_state.db, self.app_state.env)
				.await?;

		let mut networks_map = self.networks_map.lock().await;

		// drop removed networks
		let all_networks_ids: Vec<PrimaryId> =
			all_networks.iter().map(|n| n.network_id).collect();
		networks_map
			.retain(|network_id, _| all_networks_ids.contains(network_id));

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

	pub async fn index(&mut self) -> Result<()> {
		let mut last_sync_at = utils::now();
		let mut last_read_block_map = HashMap::<i64, u64>::new();
		let mut index_results = IndexResults::new();

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
				let mut network_ids = HashMap::new();
				for (network_id, chain) in networks_map.iter() {
					let last_read_block = {
						match last_read_block_map.contains_key(network_id) {
							true => last_read_block_map[network_id],
							_ => {
								let index_latest_block = Config::get::<u64>(
									&self.app_state.db,
									ConfigKey::IndexLatestBlock(
										*network_id as u64,
									),
								);

								if let Some(hit) = index_latest_block.await? {
									hit.value
								} else {
									chain.get_last_processed_block().await?
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

				// @TODO insert futures for syncing modules

				let (done, mut is_done) = mpsc::channel(network_ids.len());
				let mut futures = vec![];
				let should_keep_going = Arc::new(AtomicBool::new(true));
				let mut receipts = HashMap::<PrimaryId, Sender<()>>::new();

				for (network_id, last_read_block) in
					network_ids.clone().into_iter()
				{
					let (rtx, receipt) = oneshot::channel();
					receipts.insert(*network_id, rtx);

					futures.push(tokio::spawn({
						let network_id = *network_id;
						let chain = networks_map[&network_id].clone();
						let modules = chain.get_module_ids();
						let should_keep_going = should_keep_going.clone();
						let can_exit =
							CanExit::new(network_id, done.clone(), receipt);

						async move {
							let (block_height, index_results) = chain
								.process_blocks(
									last_read_block,
									None,
									modules,
									should_keep_going,
									can_exit,
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
								index_results,
							))
						}
					}));
				}

				drop(done); // drop the original non-cloned
				while let Some(network_id) = is_done.recv().await {
					network_ids.remove(&network_id);
					if network_ids.is_empty() {
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

				// pile up `index_results` and inc block markers
				for (network_id, last_read_block, new_data) in
					results.into_iter()
				{
					index_results += new_data;
					last_read_block_map.insert(network_id, last_read_block);
				}

				// batch save in warehouse
				if index_results.is_ready_to_commit() {
					// push to warehouse
					index_results.commit(&self.app_state.warehouse).await?;

					// commit latest saved blocks
					Config::set_many::<u64>(
						&self.app_state.db,
						last_read_block_map
							.iter()
							.map(|(network_id, last_read_block)| {
								(
									ConfigKey::IndexLatestBlock(
										(*network_id) as u64,
									),
									*last_read_block,
								)
							})
							.collect::<HashMap<ConfigKey, u64>>(),
					)
					.await?;
					last_read_block_map.clear();
				}
			}
		}
	}
}
