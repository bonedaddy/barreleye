use axum::{
	routing::{delete, get, post, put},
	Router,
};
use eyre::Result;
use std::{collections::HashMap, sync::Arc};

use barreleye_common::{
	models::{Address, JoinedTag, Network, PrimaryId, Tag},
	App,
};

mod create;
mod delete;
mod get;
mod list;
mod update;

pub fn get_routes() -> Router<Arc<App>> {
	Router::new()
		.route("/", post(create::handler))
		.route("/", get(list::handler))
		.route("/:id", get(get::handler))
		.route("/:id", put(update::handler))
		.route("/:id", delete(delete::handler))
}

pub async fn get_tags_data(
	app: Arc<App>,
	entity_ids: Vec<PrimaryId>,
) -> Result<(Vec<JoinedTag>, HashMap<PrimaryId, Vec<String>>)> {
	let tags = Tag::get_all_by_entity_ids(app.db(), entity_ids).await?;
	let mut map = HashMap::<PrimaryId, Vec<String>>::new();

	for tag in tags.iter() {
		if let Some(ids) = map.get_mut(&tag.entity_id) {
			ids.push(tag.id.clone());
		} else {
			map.insert(tag.entity_id, vec![tag.id.clone()]);
		}
	}

	Ok((tags, map))
}

pub async fn get_addresses_data(
	app: Arc<App>,
	entity_ids: Vec<PrimaryId>,
) -> Result<(Vec<Address>, HashMap<PrimaryId, Vec<String>>, Vec<Network>)> {
	let addresses = Address::get_all_by_entity_ids(app.db(), entity_ids, Some(false)).await?;

	let mut network_ids = addresses.iter().map(|a| a.network_id).collect::<Vec<PrimaryId>>();

	network_ids.sort_unstable();
	network_ids.dedup();

	let networks_map = Network::get_all_by_network_ids(app.db(), network_ids)
		.await?
		.into_iter()
		.map(|n| (n.network_id, n))
		.collect::<HashMap<PrimaryId, Network>>();

	let networks = networks_map.into_values().collect::<Vec<Network>>();

	let mut map = HashMap::<PrimaryId, Vec<String>>::new();
	for address in addresses.iter() {
		if let Some(ids) = map.get_mut(&address.entity_id) {
			ids.push(address.id.clone());
		} else {
			map.insert(address.entity_id, vec![address.id.clone()]);
		}
	}

	Ok((addresses, map, networks))
}
