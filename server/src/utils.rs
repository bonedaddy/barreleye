use eyre::Result;
use std::{
	collections::{HashMap, HashSet},
	sync::Arc,
};

use crate::{errors::ServerError, ServerResult};
use barreleye_common::{
	models::{Address, Amount, BasicModel, Entity, Network, PrimaryId, PrimaryIds},
	App,
};

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
		let entity_ids: PrimaryIds =
			Entity::get_all_by_ids(app.db(), entities.into_iter().collect()).await?.into();

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
