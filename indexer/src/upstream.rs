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
	models::{Config, ConfigKey, LabeledAddress, Link, Network, PrimaryId, Transfer},
	BlockHeight,
};

struct IndexedLinks {
	address: String,
	data: HashMap<String, HashSet<Link>>,
}

impl IndexedLinks {
	pub fn new(address: &str, links: Vec<Link>) -> Self {
		let mut s = Self { address: address.to_string(), data: HashMap::new() };
		s.push(links);
		s
	}

	pub fn get(&self, key: &str) -> Option<&HashSet<Link>> {
		self.data.get(key)
	}

	pub fn contains(&self, key: &str) -> bool {
		self.address == key || self.data.get(key).is_some()
	}

	pub fn push(&mut self, links: Vec<Link>) {
		for link in links.into_iter() {
			if let Some(set) = self.data.get_mut(&link.to_address) {
				set.insert(link);
			} else {
				self.data.insert(link.to_address.clone(), HashSet::from([link]));
			}
		}
	}
}

impl Indexer {
	pub async fn index_upstream(&self, verbose: bool) -> Result<()> {
		let mut warehouse_data = WarehouseData::new();
		let mut config_key_map = HashMap::<ConfigKey, BlockHeight>::new();
		let mut started_indexing = false;

		loop {
			if !self.app.is_leading() {
				if started_indexing {
					self.log(IndexType::Upstream, "Stopping…");
				}

				started_indexing = false;
				sleep(Duration::from_secs(1)).await;
				continue;
			}

			if !started_indexing {
				started_indexing = true;
				self.log(IndexType::Upstream, "Starting…");
			}

			// get all networks that are not syncing in chunks
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
				let map = networks
					.into_iter()
					.map(|n| (ConfigKey::IndexerTailSync(n.network_id), n.network_id))
					.collect::<HashMap<ConfigKey, PrimaryId>>();

				Config::get_many::<BlockHeight>(&self.app.db, map.clone().into_keys().collect())
					.await?
					.into_iter()
					.filter_map(|(config_key, hit)| match hit.value > 0 {
						true => map.get(&config_key).map(|&network_id| (network_id, hit.value)),
						_ => None,
					})
					.collect::<HashMap<PrimaryId, BlockHeight>>()
			};
			if block_height_map.is_empty() {
				self.log(IndexType::Upstream, "No fully-synced active networks. Waiting…");
				sleep(Duration::from_secs(5)).await;
				continue;
			}

			// fetch all labeled addresses
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
				let max_block_height = block_height_map[&network_id];

				// let max_block_height = *block_height_map.get(&network_id).unwrap();

				// get latest block for this labeled address:
				// 1. if in cache -> get it from there
				// 2. if cache is not set -> try reading from configs
				// 3. if not in configs -> fast-forward to 1st interaction from warehouse
				// 4. if not in warehouse -> no need to scan chain; set to chain height
				// 5. else -> `0`
				let config_key =
					ConfigKey::IndexerUpstreamSync(network_id, labeled_address.labeled_address_id);
				let block_height = match config_key_map.get(&config_key) {
					Some(&block_height) => block_height,
					_ => match Config::get::<BlockHeight>(&self.app.db, config_key).await? {
						Some(hit) => hit.value,
						_ => Transfer::get_first_by_source(
							&self.app.warehouse,
							network_id,
							&labeled_address.address,
						)
						.await?
						.map_or_else(
							|| match block_height_map.get(&network_id) {
								Some(&max_block_height) => max_block_height,
								_ => 0,
							},
							|t| t.block_height - 1,
						),
					},
				};

				// process a new block range if we're not at the tip
				if block_height < max_block_height {
					futures.spawn({
						let warehouse = self.app.warehouse.clone();
						let min_block_height = block_height + 1;
						let max_block_height = cmp::min(block_height + 10, max_block_height);
						let uncommitted_links = warehouse_data
							.clone()
							.links
							.into_iter()
							.filter(|l| l.network_id == network_id as u64)
							.collect::<Vec<Link>>();

						async move {
							let mut ret = WarehouseData::new();

							// seed data from processed but uncommitted links
							let mut indexed_links = IndexedLinks::new(
								&labeled_address.address.clone(),
								uncommitted_links,
							);

							// seed data from warehouse
							indexed_links.push(
								Link::get_all_for_seed_blocks(
									&warehouse,
									network_id,
									(min_block_height, max_block_height),
								)
								.await?,
							);

							// process transfers for a range of blocks
							for transfer in Transfer::get_all_by_block_range(
								&warehouse,
								network_id,
								(min_block_height, max_block_height),
							)
							.await?
							.into_iter()
							{
								if indexed_links.contains(&transfer.from_address) {
									let mut new_links = vec![];

									// create new links
									if let Some(set) = indexed_links.get(&transfer.from_address) {
										// extending branch
										for prev_link in set.iter() {
											let mut transfer_uuids =
												prev_link.transfer_uuids.clone();
											transfer_uuids.push(transfer.uuid.to_string());

											let link = Link::new(
												labeled_address.network_id,
												transfer.block_height,
												&prev_link.from_address,
												&transfer.to_address,
												transfer_uuids,
												transfer.created_at,
											);

											ret.links.insert(link.clone());
											new_links.push(link);
										}
									} else {
										// starting new branch
										let link = Link::new(
											labeled_address.network_id,
											transfer.block_height,
											&transfer.from_address,
											&transfer.to_address,
											vec![transfer.uuid.to_string()],
											transfer.created_at,
										);

										ret.links.insert(link.clone());
										new_links.push(link);
									}

									// add to indexed data
									indexed_links.push(new_links);
								}
							}

							Ok::<_, ErrReport>((config_key, max_block_height, ret))
						}
					});
				}
			}

			// if no new blocks or addresses to scan, give it a break before restarting
			let should_pause = futures.is_empty();

			// collect results
			while let Some(res) = futures.join_next().await {
				if let Ok((config_key, block_height, new_warehouse_data)) = res? {
					warehouse_data += new_warehouse_data;
					config_key_map.insert(config_key, block_height);
				}
			}

			// commit if collected enough
			if warehouse_data.should_commit() && self.app.is_leading() {
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
			if should_pause {
				sleep(Duration::from_secs(1)).await;
			}
		}
	}
}
