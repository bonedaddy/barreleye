use eyre::{ErrReport, Result};
use std::{
	cmp,
	collections::{HashMap, HashSet},
};
use tokio::{
	task::JoinSet,
	time::{sleep, Duration},
};

use crate::{IndexType, Indexer};
use barreleye_common::{
	models::{Config, ConfigKey, LabeledAddress, Link, Network, Transfer},
	BlockHeight,
};

impl Indexer {
	pub async fn index_upstream(&self) -> Result<()> {
		let mut started_indexing = false;

		loop {
			if !self.app.is_leading() {
				started_indexing = false;
				sleep(Duration::from_secs(1)).await;
				continue;
			}

			if !started_indexing {
				started_indexing = true;
				self.log(IndexType::Upstream, "Starting…");
			}

			// proceed only with non-fast-syncing networks (those that aren't chunked)
			let mut networks = vec![];
			for network in Network::get_all_by_env(&self.app.db, self.app.env).await?.into_iter() {
				let tail_is_syncing = Config::exist_by_keywords(
					&self.app.db,
					vec![format!("tail_sync_n{}", network.network_id)],
				);

				let is_actively_syncing = Config::exist_by_keywords(
					&self.app.db,
					vec![
						format!("chunk_sync_n{}", network.network_id),
						format!("module_sync_n{}", network.network_id),
					],
				);

				if network.is_active && tail_is_syncing.await? && !is_actively_syncing.await? {
					networks.push(network);
				}
			}

			// create a map of `network_id` -> `latest_block`
			let block_height_map = {
				let mut ret = HashMap::new();

				// @TODO optimize with one `Config::get_many` call
				for network in networks.into_iter() {
					let block_height = Config::get::<BlockHeight>(
						&self.app.db,
						ConfigKey::IndexerTailSync(network.network_id),
					)
					.await?
					.map(|h| h.value)
					.unwrap_or(0);

					if block_height > 0 {
						ret.insert(network.network_id, block_height);
					}
				}

				ret
			};
			if block_height_map.is_empty() {
				self.log(IndexType::Upstream, "No fully-synced active networks. Waiting…");
				sleep(Duration::from_secs(5)).await;
				continue;
			}

			// process a chunk of blocks per labeled address
			let mut futures = JoinSet::new();
			for labeled_address in LabeledAddress::get_all_by_network_ids(
				&self.app.db,
				block_height_map.clone().into_keys().collect(),
			)
			.await?
			.into_iter()
			{
				let network_id = labeled_address.network_id;

				// get either latest saved block height or skip to first interaction
				// for that particular labeled address
				let block_height = {
					let config_key = ConfigKey::IndexerUpstreamSync(
						network_id,
						labeled_address.labeled_address_id,
					);
					match Config::get::<BlockHeight>(&self.app.db, config_key).await? {
						Some(hit) => hit.value,
						_ => Transfer::get_first_by_source(
							&self.app.warehouse,
							network_id,
							&labeled_address.address,
						)
						.await?
						.map(|t| t.block_height - 1)
						.unwrap_or(0),
					}
				};

				// spawn a future if there's blocks to process
				match block_height_map.get(&network_id) {
					Some(&max_block_height) if block_height < max_block_height => {
						futures.spawn({
							let warehouse = self.app.warehouse.clone();
							let min_block_height = block_height + 1;
							let max_block_height = cmp::min(block_height + 10, max_block_height);

							async move {
								let mut new_links = HashSet::new();
								let mut tracking_addresses =
									HashSet::from([labeled_address.address.clone()]);
								let mut indexed_links: HashMap<String, HashSet<Link>> =
									HashMap::new();

								// seed links into an indexed structure
								for link in Link::get_all_for_seed_blocks(
									&warehouse,
									network_id,
									(min_block_height, max_block_height),
								)
								.await?
								.into_iter()
								{
									tracking_addresses.insert(link.to_address.clone());
									if let Some(set) = indexed_links.get_mut(&link.to_address) {
										set.insert(link);
									} else {
										indexed_links
											.insert(link.to_address.clone(), HashSet::from([link]));
									}
								}

								// process transfers for a range of blocks
								for transfer in Transfer::get_all_by_block_range(
									&warehouse,
									network_id,
									(min_block_height, max_block_height),
								)
								.await?
								.into_iter()
								{
									if tracking_addresses.contains(&transfer.from_address) {
										let mut tmp_links = vec![];

										let get_link = |tx_hashes| {
											Link::new(
												labeled_address.network_id,
												transfer.block_height,
												&transfer.from_address,
												&transfer.to_address,
												tx_hashes,
												transfer.created_at,
											)
										};

										// create new links
										if let Some(set) = indexed_links.get(&transfer.from_address)
										{
											for prev_link in set.iter() {
												// extending branch
												let mut tx_hashes = prev_link.tx_hashes.clone();
												tx_hashes.push(transfer.tx_hash.clone());

												let link = get_link(tx_hashes);

												new_links.insert(link.clone());
												tmp_links.push(link);
											}
										} else {
											// starting new branch
											let link = get_link(vec![transfer.tx_hash]);

											new_links.insert(link.clone());
											tmp_links.push(link);
										}

										// add to indexed data
										tracking_addresses.insert(transfer.to_address);
										for link in tmp_links.into_iter() {
											if let Some(set) =
												indexed_links.get_mut(&link.to_address)
											{
												set.insert(link);
											} else {
												indexed_links.insert(
													link.to_address.clone(),
													HashSet::from([link]),
												);
											}
										}
									}
								}

								// @TODO commit or send to pipe

								// @TODO update config

								// ret
								Ok::<_, ErrReport>(())
							}
						});
					}
					_ => {}
				}
			}
			while let Some(res) = futures.join_next().await {
				let _ = res?;
			}

			// @TODO
			sleep(Duration::from_secs(1)).await;
			self.log(IndexType::Upstream, "loop");
		}
	}
}
