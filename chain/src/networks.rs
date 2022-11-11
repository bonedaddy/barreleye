use eyre::{ErrReport, Result};
use futures::future::join_all;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use itertools::{Either, Itertools};
use std::{collections::HashMap, sync::Arc};
use tokio::{
	signal,
	task::JoinHandle,
	time::{sleep, Duration},
};

use crate::{Bitcoin, ChainTrait, Evm};
use barreleye_common::{
	models::{BasicModel, Network, PrimaryId},
	progress,
	progress::Step,
	AppError, AppState, Blockchain,
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
									"connected to {rpc}"
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
		let all_networks = Network::get_all(&self.app_state.db).await?;

		// add new networks
		for n in all_networks
			.iter()
			.filter_map(|network| match self.get_chain(network.network_id) {
				None => Some(network.clone()),
				_ => None,
			})
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

		// drop removed networks
		let ids: Vec<PrimaryId> =
			all_networks.iter().map(|n| n.network_id).collect();
		self.networks_map.retain(|network_id, _| ids.contains(network_id));

		Ok(())
	}

	pub async fn watch(&mut self) -> Result<()> {
		let watch = async {
			loop {
				if self.app_state.is_leader() {
					self.sync_networks().await?;

					let futures = self
						.networks_map
						.iter()
						.map(|(_, chain)| {
							tokio::spawn({
								let c = chain.clone();
								async move { c.process_blocks().await }
							})
						})
						.collect::<Vec<JoinHandle<Result<_>>>>();

					join_all(futures).await;
				}

				sleep(Duration::from_secs(1)).await;
			}
		};

		tokio::select! {
			v = watch => v,
			_ = signal::ctrl_c() => Ok(()),
		}
	}

	#[allow(clippy::borrowed_box)]
	pub fn get_chain(
		&self,
		network_id: PrimaryId,
	) -> Option<&Box<dyn ChainTrait>> {
		if self.networks_map.contains_key(&network_id) {
			Some(&self.networks_map[&network_id])
		} else {
			None
		}
	}
}
