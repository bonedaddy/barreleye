use console::style;
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
	chain::WarehouseData,
	models::{Config, ConfigKey, LabeledAddress, Link, Network, Transfer},
	BlockHeight,
};

impl Indexer {
	pub async fn index_upstream(&self, verbose: bool) -> Result<()> {
		let mut warehouse_data = WarehouseData::new();
		let mut config_key_map = HashMap::<ConfigKey, BlockHeight>::new();
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

			// proceed only with those networks that are not chunk-syncing
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

			// fetch labeled addresses
			let labeled_addresses = LabeledAddress::get_all_by_network_ids(
				&self.app.db,
				block_height_map.clone().into_keys().collect(),
			)
			.await?;
			if labeled_addresses.is_empty() {
				self.log(IndexType::Upstream, "Nothing to do (no labeled addresses)");
				sleep(Duration::from_secs(5)).await;
				continue;
			}

			// process a chunk of blocks per labeled address
			let mut futures = JoinSet::new();
			for labeled_address in labeled_addresses.into_iter() {
				let network_id = labeled_address.network_id;

				// get either latest saved block height or skip to first interaction
				// for that particular labeled address
				let config_key =
					ConfigKey::IndexerUpstreamSync(network_id, labeled_address.labeled_address_id);
				let block_height =
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
					};

				// spawn a future if there's blocks to process
				match block_height_map.get(&network_id) {
					Some(&max_block_height) if block_height < max_block_height => {
						futures.spawn({
							let warehouse = self.app.warehouse.clone();
							let min_block_height = block_height + 1;
							let max_block_height = cmp::min(block_height + 10, max_block_height);

							async move {
								let mut ret = WarehouseData::new();
								let mut tracking_addresses =
									HashSet::from([labeled_address.address.clone()]);
								let mut indexed_links = HashMap::<String, HashSet<Link>>::new();

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

												ret.links.insert(link.clone());
												tmp_links.push(link);
											}
										} else {
											// starting new branch
											let link = get_link(vec![transfer.tx_hash]);

											ret.links.insert(link.clone());
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

								Ok::<_, ErrReport>((config_key, max_block_height, ret))
							}
						});
					}
					_ => {}
				}
			}

			// collect results
			while let Some(res) = futures.join_next().await {
				if let Ok((config_key, block_height, new_warehouse_data)) = res? {
					warehouse_data += new_warehouse_data;
					config_key_map.insert(config_key, block_height);
				}
			}

			// commit if collected enough
			if warehouse_data.should_commit() {
				if verbose {
					self.log(
						IndexType::Upstream,
						&format!(
							"Pushing {} record(s) to warehouse",
							style(self.format_number(warehouse_data.len())?).bold(),
						),
					);
				}

				// push to warehouse
				warehouse_data.commit(&self.app.warehouse).await?;

				// commit config marker updates
				Config::set_many::<BlockHeight>(&self.app.db, config_key_map.clone()).await?;
				config_key_map.clear();
			}

			// if no threads ever started, pause
			if futures.is_empty() {
				sleep(Duration::from_secs(1)).await;
			}
		}
	}
}
