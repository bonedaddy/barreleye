use clap::{builder, ValueEnum};
use derive_more::Display;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use std::{
	str::FromStr,
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
};

pub use address::Address;
pub use cache::Cache;
pub use db::Db;
pub use errors::AppError;
pub use settings::Settings;
pub use warehouse::Warehouse;

pub mod address;
pub mod cache;
pub mod db;
pub mod errors;
pub mod models;
pub mod progress;
pub mod settings;
pub mod u256;
pub mod utils;
pub mod warehouse;

#[derive(Clone)]
pub struct AppState {
	pub uuid: Uuid,
	pub settings: Arc<Settings>,
	pub cache: Arc<Cache>,
	pub db: Arc<Db>,
	pub warehouse: Arc<Warehouse>,
	pub env: Env,
	pub is_indexer: bool,
	pub is_server: bool,
	is_ready: Arc<AtomicBool>,
	is_leader: Arc<AtomicBool>,
}

impl AppState {
	pub fn new(
		settings: Arc<Settings>,
		cache: Arc<Cache>,
		db: Arc<Db>,
		warehouse: Arc<Warehouse>,
		env: Env,
		is_indexer: bool,
		is_server: bool,
	) -> Self {
		AppState {
			is_indexer,
			is_server,
			is_ready: Arc::new(AtomicBool::new(false)),
			is_leader: Arc::new(AtomicBool::new(false)),
			uuid: utils::new_uuid(),
			settings,
			cache,
			db,
			warehouse,
			env,
		}
	}

	pub fn is_ready(&self) -> bool {
		self.is_ready.load(Ordering::SeqCst)
	}

	pub fn set_is_ready(&self) {
		self.is_ready.store(true, Ordering::SeqCst);
	}

	pub fn is_leader(&self) -> bool {
		self.is_leader.load(Ordering::SeqCst)
	}

	pub fn set_is_leader(&self, is_leader: bool) {
		self.is_leader.store(is_leader, Ordering::SeqCst);
	}
}

#[derive(Display, Debug, Serialize, Deserialize)]
pub enum IdPrefix {
	#[display(fmt = "net")]
	Network,
	#[display(fmt = "key")]
	ApiKey,
	#[display(fmt = "lab")]
	Label,
	#[display(fmt = "lab_adr")]
	LabeledAddress,
}

#[derive(Display, Debug, PartialEq, Eq)]
pub enum LabelId {
	#[display(fmt = "lab_ofac")]
	Ofac,
	#[display(fmt = "lab_ofsi")]
	Ofsi,
}

impl FromStr for LabelId {
	type Err = ();
	fn from_str(id: &str) -> Result<LabelId, Self::Err> {
		match id {
			"lab_ofac" => Ok(LabelId::Ofac),
			"lab_ofsi" => Ok(LabelId::Ofsi),
			_ => Err(()),
		}
	}
}

#[derive(
	Debug,
	EnumIter,
	DeriveActiveEnum,
	Copy,
	Clone,
	PartialEq,
	Eq,
	Serialize,
	Deserialize,
)]
#[sea_orm(rs_type = "i16", db_type = "SmallInteger")]
pub enum Env {
	#[serde(rename = "localhost")]
	Localhost = 1,
	#[serde(rename = "testnet")]
	Testnet = 2,
	#[serde(rename = "mainnet")]
	Mainnet = 3,
}

impl ValueEnum for Env {
	fn value_variants<'a>() -> &'a [Self] {
		&[Self::Localhost, Self::Testnet, Self::Mainnet]
	}

	fn to_possible_value<'a>(&self) -> Option<builder::PossibleValue> {
		match self {
			Self::Localhost => Some(builder::PossibleValue::new("localhost")),
			Self::Testnet => Some(builder::PossibleValue::new("testnet")),
			Self::Mainnet => Some(builder::PossibleValue::new("mainnet")),
		}
	}
}

#[derive(
	Debug,
	EnumIter,
	DeriveActiveEnum,
	Copy,
	Clone,
	PartialEq,
	Eq,
	Serialize,
	Deserialize,
)]
#[sea_orm(rs_type = "i16", db_type = "SmallInteger")]
pub enum Blockchain {
	#[serde(rename = "bitcoin")]
	Bitcoin = 1,
	#[serde(rename = "evm")]
	Evm = 2,
}

#[derive(Serialize, Deserialize)]
pub enum Risk {
	#[serde(rename = "LOW")]
	Low = 10,
	#[serde(rename = "MEDIUM")]
	Medium = 20,
	#[serde(rename = "HIGH")]
	High = 30,
	#[serde(rename = "SEVERE")]
	Severe = 40,
}
