use eyre::Result;
use regex::Regex;
use std::{
	collections::{HashMap, HashSet},
	str::FromStr,
	sync::Arc,
};
use tokio::{
	signal,
	time::{sleep, Duration},
};

use barreleye_common::{
	models::{BasicModel, Config, ConfigKey, Label, LabeledAddress},
	utils, Address, AppState, LabelId,
};

pub struct Lists {
	app_state: Arc<AppState>,
}

impl Lists {
	pub fn new(app_state: Arc<AppState>) -> Self {
		Self { app_state }
	}

	pub async fn watch(&self) -> Result<()> {
		let watch = async {
			loop {
				let timeout = if self.app_state.is_ready() &&
					self.app_state.is_leader()
				{
					self.fetch_data().await?;
					self.app_state.settings.sdn_refresh_rate
				} else {
					1
				};

				sleep(Duration::from_secs(timeout)).await;
			}
		};

		tokio::select! {
			v = watch => v,
			_ = signal::ctrl_c() => Ok(()),
		}
	}

	async fn fetch_data(&self) -> Result<()> {
		let stale_at =
			utils::ago_in_seconds(self.app_state.settings.sdn_refresh_rate);

		let labels =
			Label::get_all_enabled_and_hardcoded(&self.app_state.db).await?;

		// skips labels that have been recently fetched
		let mut label_ids = vec![];
		for label in labels.iter() {
			match Config::get::<u8>(
				&self.app_state.db,
				ConfigKey::LabelFetched(label.label_id),
			)
			.await?
			{
				None => label_ids.push(label.label_id),
				Some(hit) if hit.updated_at < stale_at => {
					label_ids.push(label.label_id)
				}
				_ => {}
			}
		}
		if label_ids.is_empty() {
			return Ok(());
		}

		let labeled_addresses =
			LabeledAddress::get_all_by_label_ids(&self.app_state.db, label_ids)
				.await?;

		// lab_ofac => {
		//     lab_adr_123 => 'addr1',
		//     lab_adr_456 => 'addr2',
		// },
		// lab_ofsi => {
		//     lab_adr_789 => 'addr1',
		// }
		let existing_data: HashMap<String, HashMap<String, String>> = labels
			.iter()
			.map(|l| {
				(
					l.id.clone(),
					labeled_addresses
						.iter()
						.filter_map(|la| match la.label_id == l.label_id {
							true => Some((la.id.clone(), la.address.clone())),
							_ => None,
						})
						.collect(),
				)
			})
			.collect();

		for label in labels.iter() {
			let fresh_addresses = match LabelId::from_str(&label.id) {
				Ok(LabelId::Ofac) => self.get_ofac_addresses().await?,
				Ok(LabelId::Ofsi) => self.get_ofsi_addresses().await?,
				_ => vec![],
			};

			// timestamp the request
			Config::set::<u8>(
				&self.app_state.db,
				ConfigKey::LabelFetched(label.label_id),
				1,
			)
			.await?;

			// add addresses that don't exist in db yet
			let mut addresses_to_add = HashSet::new();
			let existing_addresses: Vec<String> =
				existing_data[&label.id].clone().into_values().collect();
			for address in fresh_addresses.iter() {
				let item = (label.label_id, address.to_string());
				if !existing_addresses.contains(address) &&
					!addresses_to_add.contains(&item)
				{
					addresses_to_add.insert(item);
				}
			}
			if !addresses_to_add.is_empty() {
				LabeledAddress::create_many(
					&self.app_state.db,
					addresses_to_add
						.iter()
						.map(|(label_id, address)| {
							LabeledAddress::new_model(
								*label_id,
								Address::new(address),
							)
						})
						.collect(),
				)
				.await?;
			}

			// remove addresses that have been cleared from the list
			let mut ids_to_delete = vec![];
			for (labeled_address_id, address) in &existing_data[&label.id] {
				if !fresh_addresses.contains(address) {
					ids_to_delete.push(labeled_address_id.to_string());
				}
			}
			if !ids_to_delete.is_empty() {
				LabeledAddress::delete_by_ids(
					&self.app_state.db,
					ids_to_delete,
				)
				.await?;
			}
		}

		Ok(())
	}

	async fn get_ofac_addresses(&self) -> Result<Vec<String>> {
		self.regex_extract(
			"https://www.treasury.gov/ofac/downloads/sdn.pip",
			r"Digital\s+Currency\s+Address\s*-\s*([0-9a-zA-Z]+)\s+([0-9a-zA-Z]+);",
		)
		.await
	}

	async fn get_ofsi_addresses(&self) -> Result<Vec<String>> {
		self.regex_extract(
            "https://ofsistorage.blob.core.windows.net/publishlive/2022format/ConList.csv",
            r"Digital\s+Currency\s+Address\s*:\s*([0-9a-zA-Z]+)\s+([0-9a-zA-Z]+)",
        ).await
	}

	async fn regex_extract(
		&self,
		url: &str,
		regex: &str,
	) -> Result<Vec<String>> {
		Ok(Regex::new(regex)?
			.captures_iter(&reqwest::get(url).await?.text().await?)
			.filter_map(|cap| cap.get(2).map(|v| v.as_str().to_lowercase()))
			.collect::<Vec<String>>())
	}
}
