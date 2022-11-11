use eyre::{ErrReport, Result};
use futures::future::join_all;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use itertools::{Either, Itertools};
use std::{collections::HashMap, sync::Arc};
use tokio::{
	signal,
	time::{sleep, Duration},
};

use crate::{Bitcoin, ChainTrait, Evm};
use barreleye_common::{
	models::{BasicModel, Leader, Network, PrimaryId},
	progress,
	progress::Step,
	utils, AppError, AppState, Blockchain,
};

pub struct Networks {
	app_state: Arc<AppState>,
	map_network_id_to_chain: HashMap<PrimaryId, Arc<Box<dyn ChainTrait>>>,
}

impl Networks {
	pub fn new(app_state: Arc<AppState>) -> Self {
		Self { app_state, map_network_id_to_chain: HashMap::new() }
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
							let boxed_chain: Box<dyn ChainTrait> =
								match n.blockchain {
									Blockchain::Bitcoin => Box::new(
										Bitcoin::new(app_state, n, &pb).await?,
									),
									Blockchain::Evm => Box::new(
										Evm::new(app_state, n, &pb).await?,
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

		let (map_network_id_to_chain, failures): (HashMap<_, _>, Vec<_>) =
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

		self.map_network_id_to_chain = map_network_id_to_chain;

		Ok(self)
	}

	pub async fn watch(&self) -> Result<()> {
		Leader::truncate(
			&self.app_state.db,
			31_557_600 +
				self.app_state.settings.warehouse.processing_frequency +
				self.app_state.settings.warehouse.leader_promotion_timeout,
		)
		.await?;

		tokio::select! {
			_ = self.leader_check() => {},
			_ = signal::ctrl_c() => {},
		}

		Ok(())
	}

	async fn leader_check(&self) -> Result<()> {
		loop {
			let frequency =
				self.app_state.settings.warehouse.processing_frequency;
			let promotion_at = utils::ago_in_seconds(
				self.app_state.settings.warehouse.leader_promotion_timeout,
			);

			match Leader::get_active(&self.app_state.db, frequency + 1).await? {
				Some(leader) if leader.uuid == self.app_state.uuid => {
					Leader::check_in(&self.app_state.db, self.app_state.uuid)
						.await?;

					for (_, chain) in self.map_network_id_to_chain.iter() {
						tokio::spawn({
							let chain = chain.clone();
							async move { chain.process_blocks().await }
						});
					}
				}
				None => {
					let promote = Leader::create(
						&self.app_state.db,
						Leader::new_model(self.app_state.uuid),
					);

					match Leader::get_last(&self.app_state.db).await? {
						Some(leader) if leader.updated_at < promotion_at => {
							promote.await?;
						}
						None => {
							promote.await?;
						}
						_ => {}
					}
				}
				_ => {}
			}

			sleep(Duration::from_secs(frequency)).await
		}
	}

	#[allow(clippy::borrowed_box)]
	pub fn get_chain(
		&self,
		network_id: PrimaryId,
	) -> Option<&Box<dyn ChainTrait>> {
		if self.map_network_id_to_chain.contains_key(&network_id) {
			Some(&self.map_network_id_to_chain[&network_id])
		} else {
			None
		}
	}

	#[allow(clippy::borrowed_box)]
	pub fn get_by_blockchain_and_chain_id(
		&self,
		blockchain: Blockchain,
		chain_id: u64,
	) -> Option<&Box<dyn ChainTrait>> {
		for (network_id, b) in self.map_network_id_to_chain.iter() {
			let network = b.get_network();
			if network.blockchain == blockchain &&
				network.chain_id == chain_id as PrimaryId
			{
				return Some(&self.map_network_id_to_chain[network_id]);
			}
		}

		None
	}
}
