use eyre::Result;
use log::info;

use barreleye_common::{
	db,
	models::{BasicModel, SanctionedAddress},
};

mod lists;

#[tokio::main]
pub async fn start() -> Result<()> {
	let db = db::new().await?;

	if let Ok(sanctioned_addresses) = lists::get_data().await {
		for sanctioned_address in sanctioned_addresses.into_iter() {
			SanctionedAddress::try_create(&db, sanctioned_address).await?;
		}

		let count = SanctionedAddress::count_all(&db).await?;
		info!("Updated");
		info!("{count} total record(s)");
	} else {
		info!("Could not fetch data");
	}

	Ok(())
}
