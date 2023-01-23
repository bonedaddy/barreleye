use eyre::Result;
use std::{
	collections::{HashMap, HashSet},
	sync::Arc,
};

use crate::{errors::ServerError, ServerResult};
use barreleye_common::{
	models::{Address, Amount, BasicModel, Entity, EntityTag, Network, PrimaryId, Tag},
	App, Aux,
};

type IdMap = HashMap<PrimaryId, Vec<String>>;

pub async fn get_tags_data(
	app: Arc<App>,
	aux: Vec<Aux>,
	entity_ids: Vec<PrimaryId>,
) -> Result<Option<(Vec<Tag>, IdMap)>> {
	Ok(if aux.contains(&Aux::Tags) {
		let mut tags = vec![];
		let mut map = IdMap::new();

		let entity_tags = EntityTag::get_all_by_entity_ids(app.db(), entity_ids).await?;
		if !entity_tags.is_empty() {
			let tag_ids = entity_tags.iter().map(|et| et.tag_id).collect::<Vec<PrimaryId>>();
			tags = Tag::get_all_by_tag_ids(app.db(), tag_ids).await?;

			let tags_map = tags
				.iter()
				.map(|t| (t.tag_id, t.id.clone()))
				.collect::<HashMap<PrimaryId, String>>();

			for entity_tag in entity_tags.iter() {
				if let Some(id) = tags_map.get(&entity_tag.tag_id) {
					if let Some(ids) = map.get_mut(&entity_tag.entity_id) {
						ids.push(id.clone());
					} else {
						map.insert(entity_tag.entity_id, vec![id.clone()]);
					}
				}
			}
		}

		Some((tags, map))
	} else {
		None
	})
}

pub async fn get_addresses_data(
	app: Arc<App>,
	aux: Vec<Aux>,
	entity_ids: Vec<PrimaryId>,
) -> Result<(Option<(Vec<Address>, IdMap)>, Option<Vec<Network>>)> {
	let mut addresses_data = None;
	let mut networks = None;

	if aux.contains(&Aux::Addresses) {
		let mut addresses =
			Address::get_all_by_entity_ids(app.db(), entity_ids, Some(false)).await?;

		let mut network_ids = addresses.iter().map(|a| a.network_id).collect::<Vec<PrimaryId>>();

		network_ids.sort_unstable();
		network_ids.dedup();

		if aux.contains(&Aux::Networks) {
			let networks_map = Network::get_all_by_network_ids(app.db(), network_ids)
				.await?
				.into_iter()
				.map(|n| (n.network_id, n))
				.collect::<HashMap<PrimaryId, Network>>();

			for address in addresses.iter_mut() {
				if let Some(network) = networks_map.get(&address.network_id) {
					address.network = Some(network.id.clone());
				}
			}

			networks = Some(networks_map.into_values().collect::<Vec<Network>>());
		}

		let mut map = IdMap::new();
		for address in addresses.iter() {
			if let Some(ids) = map.get_mut(&address.entity_id) {
				ids.push(address.id.clone());
			} else {
				map.insert(address.entity_id, vec![address.id.clone()]);
			}
		}

		addresses_data = Some((addresses, map));
	}

	Ok((addresses_data, networks))
}

pub async fn get_addresses_from_params(
	app: Arc<App>,
	addresses: Vec<String>,
	entities: Vec<String>,
) -> ServerResult<Vec<String>> {
	let mut ret = HashSet::new();

	let addresses = HashSet::<String>::from_iter(addresses.iter().cloned());
	let entities = HashSet::<String>::from_iter(entities.iter().cloned());
	let max_limit = 100;

	// validation
	if addresses.len() > max_limit {
		return Err(ServerError::ExceededLimit { field: "address".to_string(), limit: max_limit });
	}
	if entities.len() > max_limit {
		return Err(ServerError::ExceededLimit { field: "entity".to_string(), limit: max_limit });
	}

	// add addresses
	for address in addresses.into_iter() {
		if !address.is_empty() {
			let formatted_address = app.format_address(&address).await?;
			ret.insert(formatted_address);
		}
	}

	// add addresses from entities
	if !entities.is_empty() {
		let entity_ids = Entity::get_all_by_ids(app.db(), entities.into_iter().collect())
			.await?
			.into_iter()
			.map(|e| e.entity_id)
			.collect::<Vec<PrimaryId>>();

		if !entity_ids.is_empty() {
			for address in Address::get_all_by_entity_ids(app.db(), entity_ids, Some(false)).await?
			{
				ret.insert(address.address);
			}
		}
	}

	if ret.is_empty() {
		return Err(ServerError::MissingInputParams);
	}

	Ok(ret.into_iter().collect::<Vec<String>>())
}

pub async fn get_networks(app: Arc<App>, addresses: Vec<String>) -> Result<Vec<Network>> {
	let mut ret = vec![];

	let n = app.networks.read().await;
	let network_ids = Amount::get_all_network_ids_by_addresses(&app.warehouse, addresses).await?;
	if !network_ids.is_empty() {
		for (_, chain) in n.iter().filter(|(network_id, _)| network_ids.contains(network_id)) {
			ret.push(chain.get_network());
		}
	}

	Ok(ret)
}

pub fn extract_primary_ids(
	field: &str,
	mut ids: Vec<String>,
	map: HashMap<String, PrimaryId>,
) -> ServerResult<Vec<PrimaryId>> {
	if !ids.is_empty() {
		ids.sort_unstable();
		ids.dedup();

		let invalid_ids = ids
			.into_iter()
			.filter_map(|tag_id| if !map.contains_key(&tag_id) { Some(tag_id) } else { None })
			.collect::<Vec<String>>();

		if !invalid_ids.is_empty() {
			return Err(ServerError::InvalidValues {
				field: field.to_string(),
				values: invalid_ids.join(", "),
			});
		}

		return Ok(map.into_values().collect());
	}

	Ok(vec![])
}
