use derive_more::Display;
use serde::{Deserialize, Serialize};

pub mod db;
pub mod errors;
pub mod models;
pub mod progress;
pub mod utils;

pub mod settings;
pub use settings::Settings;

#[derive(Display, Debug, Serialize, Deserialize)]
pub enum IdPrefix {
	#[display(fmt = "san_adr")]
	#[serde(rename = "san_adr")]
	SanctionedAddress,
}
