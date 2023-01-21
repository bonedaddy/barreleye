use eyre::Result;
use std::{
	collections::{HashMap, HashSet},
	sync::Arc,
};

use crate::{errors::ServerError, ServerResult};
use barreleye_common::{
	models::{Address, Amount, BasicModel, Entity, Network, PrimaryId},
	App,
};

pub async fn get_addresses_from_params(
	app: Arc<App>,
	address: Option<String>,
	entity: Option<String>,
) -> ServerResult<Vec<String>> {
	let mut ret = HashSet::new();

	if let Some(address) = address {
		if !address.is_empty() {
			let formatted_address = app.format_address(&address).await?;
			ret.insert(formatted_address);
		}
	}

	if let Some(entity_id) = entity {
		match Entity::get_by_id(app.db(), &entity_id).await? {
			Some(entity) => {
				let addresses =
					Address::get_all_by_entity_ids(app.db(), vec![entity.entity_id], Some(false))
						.await?;
				for address in addresses {
					ret.insert(address.address);
				}
			}
			_ => {
				return Err(ServerError::InvalidParam {
					field: "entity".to_string(),
					value: entity_id,
				})
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
