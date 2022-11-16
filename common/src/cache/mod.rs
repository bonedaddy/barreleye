use async_trait::async_trait;
use derive_more::Display;
use eyre::{Result, WrapErr};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use crate::{cache::rocksdb::RocksDb, Settings};

mod rocksdb;

#[derive(Display, Debug, Clone)]
pub enum CacheKey {
	#[display(fmt = "evm_smart_contract_{}_{}", "_0", "_1")]
	EvmSmartContract(u64, String),
	#[display(fmt = "bitcoin_txindex_{}_{}_{}", "_0", "_1", "_2")]
	BitcoinTxIndex(u64, String, u32),
}

impl From<CacheKey> for String {
	fn from(cache_key: CacheKey) -> String {
		cache_key.to_string()
	}
}

#[derive(Display, Debug, Serialize, Deserialize)]
pub enum Driver {
	#[display(fmt = "RocksDB")]
	#[serde(rename = "rocksdb")]
	RocksDB,
}

#[async_trait]
pub trait CacheTrait: Send + Sync {
	async fn set(&self, cache_key: &str, value: &str) -> Result<()>;
	async fn get(&self, cache_key: &str) -> Result<Option<String>>;
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
				Driver::RocksDB => Box::new(
					RocksDb::new(settings).await.wrap_err(rocksdb_url)?,
				),
			},
		})
	}

	pub async fn set<T>(&self, cache_key: CacheKey, value: T) -> Result<()>
	where
		T: Serialize,
	{
		let key = cache_key.to_string().to_lowercase();
		let serialized = json!(value).to_string();
		self.cache.set(&key, &serialized).await
	}

	pub async fn get<T>(&self, cache_key: CacheKey) -> Result<Option<T>>
	where
		T: DeserializeOwned,
	{
		let key = cache_key.to_string().to_lowercase();
		Ok(self
			.cache
			.get(&key)
			.await?
			.and_then(|v| serde_json::from_str(&v).ok()))
	}

	pub async fn delete(&self, cache_key: CacheKey) -> Result<()> {
		let key = cache_key.to_string().to_lowercase();
		self.cache.delete(&key).await
	}
}
