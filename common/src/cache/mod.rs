use async_trait::async_trait;
use derive_more::Display;
use eyre::{Result, WrapErr};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{path::Path, sync::Arc};

use crate::{cache::rocksdb::RocksDb, Settings};

mod rocksdb;

#[derive(Display, Debug, Clone)]
pub enum CacheKey {
	#[display(fmt = "ex:{_0}:{_1}")]
	EvmSmartContract(u64, String), /* (network_id, address) ->
	                                * is_smart_contract: bool */
	#[display(fmt = "bx:{_0}:{_1}")]
	BitcoinTxIndex(u64, String), // (network_id, txid) -> block_height: u64
}

impl From<CacheKey> for String {
	fn from(cache_key: CacheKey) -> String {
		cache_key.to_string()
	}
}

#[derive(Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
pub enum Driver {
	#[default]
	#[serde(rename = "rocksdb")]
	RocksDB,
}

#[async_trait]
pub trait CacheTrait: Send + Sync {
	fn is_path_valid(path: &Path) -> Result<bool>
	where
		Self: Sized;
	async fn set(&self, cache_key: &str, value: &[u8]) -> Result<()>;
	async fn get(&self, cache_key: &str) -> Result<Option<Vec<u8>>>;
	async fn delete(&self, cache_key: &str) -> Result<()>;
}

pub struct Cache {
	settings: Arc<Settings>,
	cache: Box<dyn CacheTrait>,
}

impl Cache {
	pub async fn new(settings: Arc<Settings>) -> Result<Self> {
		Ok(Self {
			settings: settings.clone(),
			cache: Box::new(
				RocksDb::new(&settings.indexer_cache_dir, true)
					.await
					.wrap_err(settings.indexer_cache_dir.display().to_string())?,
			),
		})
	}

	pub fn is_path_valid(driver: Driver, path: &Path) -> Result<bool> {
		match driver {
			Driver::RocksDB => RocksDb::is_path_valid(path),
		}
	}

	pub async fn set_read_only(&mut self, is_read_only: bool) -> Result<()> {
		self.cache = Box::new(RocksDb::new(&self.settings.indexer_cache_dir, is_read_only).await?);
		Ok(())
	}

	pub async fn set<T>(&self, cache_key: CacheKey, value: T) -> Result<()>
	where
		T: Serialize,
	{
		let key = cache_key.to_string();
		let value = rmp_serde::to_vec(&value)?;

		self.cache.set(&key, &value).await
	}

	pub async fn get<T>(&self, cache_key: CacheKey) -> Result<Option<T>>
	where
		T: DeserializeOwned,
	{
		let key = cache_key.to_string();
		Ok(self.cache.get(&key).await?.and_then(|v| rmp_serde::from_slice(&v).ok()))
	}

	pub async fn delete(&self, cache_key: CacheKey) -> Result<()> {
		let key = cache_key.to_string();
		self.cache.delete(&key).await
	}
}
