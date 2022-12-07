use async_trait::async_trait;
use derive_more::Display;
use eyre::{Result, WrapErr};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::sync::Arc;

use crate::{cache::rocksdb::RocksDb, Settings};

mod rocksdb;

#[derive(Display, Debug, Clone)]
pub enum CacheKey {
	#[display(fmt = "ex:{}:{}", "_0", "_1")]
	EvmSmartContract(u64, String), /* (network_id, address) ->
	                                * is_smart_contract: bool */
	#[display(fmt = "bx:{}:{}", "_0", "_1")]
	BitcoinTxIndex(u64, String), // (network_id, txid) -> block_height: u64
}

impl From<CacheKey> for String {
	fn from(cache_key: CacheKey) -> String {
		cache_key.to_string()
	}
}

#[derive(Display, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum Driver {
	#[display(fmt = "RocksDB")]
	#[serde(rename = "rocksdb")]
	RocksDB,
}

#[async_trait]
pub trait CacheTrait: Send + Sync {
	async fn set(&self, cache_key: &str, value: &[u8]) -> Result<()>;
	async fn get(&self, cache_key: &str) -> Result<Option<Vec<u8>>>;
	async fn delete(&self, cache_key: &str) -> Result<()>;
}

pub struct Cache {
	cache: Box<dyn CacheTrait>,
}

impl Cache {
	pub async fn new(settings: Arc<Settings>) -> Result<Self> {
		let rocksdb_url = settings.dsn.rocksdb.clone();

		Ok(Self {
			cache: match settings.cache.driver {
				Driver::RocksDB => Box::new(RocksDb::new(settings).await.wrap_err(rocksdb_url)?),
			},
		})
	}

	pub async fn set<T>(&self, cache_key: CacheKey, value: T) -> Result<()>
	where
		T: Serialize,
	{
		let key = cache_key.to_string().to_lowercase();
		let value = rmp_serde::to_vec(&value)?;

		self.cache.set(&key, &value).await
	}

	pub async fn get<T>(&self, cache_key: CacheKey) -> Result<Option<T>>
	where
		T: DeserializeOwned,
	{
		let key = cache_key.to_string().to_lowercase();
		Ok(self.cache.get(&key).await?.and_then(|v| rmp_serde::from_slice(&v).ok()))
	}

	pub async fn delete(&self, cache_key: CacheKey) -> Result<()> {
		let key = cache_key.to_string().to_lowercase();
		self.cache.delete(&key).await
	}
}
