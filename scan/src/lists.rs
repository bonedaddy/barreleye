use eyre::Result;
use regex::Regex;

use barreleye_common::models::{
	SanctionedAddress, SanctionedAddressActiveModel,
};

async fn regex_extract(
	source: &str,
	url: &str,
	regex: &str,
) -> Result<Vec<SanctionedAddressActiveModel>> {
	let data = reqwest::get(url).await?.text().await?;
	let re = Regex::new(regex)?;

	let addresses: Vec<SanctionedAddressActiveModel> = re
		.captures_iter(&data)
		.filter_map(|cap| match (cap.get(1), cap.get(2)) {
			(Some(symbol), Some(address)) => {
				let sanctioned_address = SanctionedAddress::new_model(
					source,
					&address.as_str().to_lowercase(),
					&symbol.as_str().to_lowercase(),
				)
				.ok()?;

				Some(sanctioned_address)
			}
			_ => None,
		})
		.collect();

	Ok(addresses)
}

pub async fn get_data() -> Result<Vec<SanctionedAddressActiveModel>> {
	let mut addresses = vec![];

	let (us_addresses, uk_addresses) =
		tokio::join!(
            regex_extract(
				"ofac",
                "https://www.treasury.gov/ofac/downloads/sdn.pip",
                r"Digital\s+Currency\s+Address\s*-\s*([0-9a-zA-Z]+)\s+([0-9a-zA-Z]+);",
            ),
            regex_extract(
				"ofsi",
                "https://ofsistorage.blob.core.windows.net/publishlive/2022format/ConList.csv",
                r"Digital\s+Currency\s+Address\s*:\s*([0-9a-zA-Z]+)\s+([0-9a-zA-Z]+)",
            ),
        );

	addresses.extend(us_addresses?);
	addresses.extend(uk_addresses?);

	Ok(addresses)
}
