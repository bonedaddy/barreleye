use async_trait::async_trait;
use eyre::Result;
use rocksdb::{DBCompactionStyle, DBWithThreadMode, LogLevel, MultiThreaded, Options};
use std::path::Path;

use crate::cache::CacheTrait;

pub struct RocksDb {
	db: DBWithThreadMode<MultiThreaded>,
}

impl RocksDb {
	pub async fn new(path: &Path, is_read_only: bool) -> Result<Self> {
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
		opts.set_disable_auto_compactions(false);
		opts.set_log_level(LogLevel::Warn);

		let db = if is_read_only {
			DBWithThreadMode::<MultiThreaded>::open_for_read_only(&opts, path, false)?
		} else {
			DBWithThreadMode::<MultiThreaded>::open(&opts, path)?
		};

		Ok(Self { db })
	}
}

#[async_trait]
impl CacheTrait for RocksDb {
	fn is_path_valid(path: &Path) -> Result<bool> {
		let mut opts = Options::default();
		opts.create_if_missing(true);

		Ok(DBWithThreadMode::<MultiThreaded>::open(&opts, path).is_ok())
	}

	async fn set(&self, key: &str, value: &[u8]) -> Result<()> {
		Ok(self.db.put(key, value)?)
	}

	async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
		Ok(self.db.get(key)?)
	}

	async fn delete(&self, key: &str) -> Result<()> {
		Ok(self.db.delete(key)?)
	}
}
