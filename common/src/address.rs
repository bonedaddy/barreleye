use derive_more::Display;
use ethers::{types::H160, utils};
use std::string::String;

#[derive(Display, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Address {
	pub address: String,
}

impl Address {
	pub fn new(address: &str) -> Self {
		Address { address: address.to_string() }
	}

	pub fn blank() -> Self {
		Self::new("")
	}
}

impl From<String> for Address {
	fn from(address: String) -> Address {
		Address::new(&address)
	}
}

impl From<H160> for Address {
	fn from(address: H160) -> Address {
		Address::new(&utils::to_checksum(&address, None))
	}
}

impl From<Address> for H160 {
	fn from(a: Address) -> H160 {
		if a.address.is_empty() {
			H160::zero()
		} else {
			a.address[2..].parse().unwrap()
		}
	}
}
