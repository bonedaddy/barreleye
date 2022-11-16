use async_trait::async_trait;
use eyre::Result;
use rocksdb::{
	DBCompactionStyle, DBWithThreadMode, LogLevel, MultiThreaded, Options,
};
use std::sync::Arc;

use crate::{cache::CacheTrait, utils, Settings};

pub struct RocksDb {
	db: DBWithThreadMode<MultiThreaded>,
}

impl RocksDb {
	pub async fn new(settings: Arc<Settings>) -> Result<Self> {
		let mut opts = Options::default();

		opts.create_if_missing(true);
		opts.set_max_open_files(10_000);
		opts.set_use_fsync(false);
		opts.set_bytes_per_sync(8_388_608);
		opts.optimize_for_point_lookup(1_024);
		opts.set_table_cache_num_shard_bits(6);
		opts.set_max_write_buffer_number(32);
		opts.set_write_buffer_size(536_870_912);
		opts.set_target_file_size_base(1_073_741_824);
		opts.set_min_write_buffer_number_to_merge(4);
		opts.set_level_zero_stop_writes_trigger(2_000);
		opts.set_level_zero_slowdown_writes_trigger(0);
		opts.set_compaction_style(DBCompactionStyle::Universal);
		opts.set_disable_auto_compactions(true);
		opts.set_log_level(LogLevel::Warn);

		let db = DBWithThreadMode::<MultiThreaded>::open(
			&opts,
			utils::get_db_path(&settings.dsn.rocksdb),
		)?;

		Ok(Self { db })
	}
}

#[async_trait]
impl CacheTrait for RocksDb {
	async fn set(&self, key: &str, value: &str) -> Result<()> {
		Ok(self.db.put(key, rmp_serde::to_vec(value)?)?)
	}

	async fn get(&self, key: &str) -> Result<Option<String>> {
		Ok(self.db.get(key)?.and_then(|v| rmp_serde::from_slice(&v).ok()))
	}

	async fn delete(&self, key: &str) -> Result<()> {
		Ok(self.db.delete(key)?)
	}
}
