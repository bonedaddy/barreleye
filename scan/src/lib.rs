use eyre::Result;
use log::info;
use sea_orm::DatabaseConnection;

use barreleye_common::{
	db,
	models::{BasicModel, SanctionedAddress},
};

mod lists;

pub async fn update_lists(db: &DatabaseConnection) -> Result<Option<i64>> {
	if let Ok(sanctioned_addresses) = lists::get_data().await {
		for sanctioned_address in sanctioned_addresses.into_iter() {
			SanctionedAddress::try_create(db, sanctioned_address).await?;
		}

		let count = SanctionedAddress::count_all(db).await?;
		return Ok(Some(count));
	}

	Ok(None)
}

#[tokio::main]
pub async fn start() -> Result<()> {
	let db = db::new(true).await?;

	info!("Fetching data…");
	if let Some(count) = update_lists(&db).await? {
		info!("Updated; {count} total record(s)");
	} else {
		info!("Could not pull new data");
	}

	Ok(())
}
