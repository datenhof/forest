use crate::dataconfig::{DataConfig, DataConfigEntry};
use crate::shadow::{
    Shadow, ShadowError, ShadowSerializationError, StateUpdateDocument,
};
use crate::models::{DeviceMetadata, ShadowName, TenantId};
use crate::timeseries::{
    MetricTimeSeries, MetricValue, TimeSeriesConversions, TimeseriesSerializationError,
};
use rocksdb::backup::{BackupEngine, BackupEngineOptions};
use rocksdb::Env;
pub use rocksdb::{OptimisticTransactionDB, Options};
use serde::{Deserialize, Serialize};
use tracing::warn;
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;

const MAX_FUTURE_SECONDS: u64 = 60 * 60 * 24 * 365;

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("RocksDB Error: {0}")]
    RocksDBError(#[from] rocksdb::Error),
    #[error("TimeseriesSerialization Error: {0}")]
    TimeseriesSerializationError(#[from] TimeseriesSerializationError),
    #[error("DatabaseConnection Error")]
    DatabaseConnectionError,
    #[error("Invalid Key: {0}")]
    InvalidKeyError(String),
    #[error("Bincode Error: {0}")]
    BincodeError(Box<bincode::ErrorKind>),
    #[error("DatabaseValue Error: {0}")]
    DatabaseValueError(String),
    #[error("ShadowSerialization Error: {0}")]
    ShadowSerializationError(#[from] ShadowSerializationError),
    #[error("Shadow Error: {0}")]
    ShadowError(#[from] ShadowError),
    #[error("DatabaseTransaction Error {0}")]
    DatabaseTransactionError(String),
    #[error("NotFound Error {0}")]
    NotFoundError(String),
}

impl From<Box<bincode::ErrorKind>> for DatabaseError {
    fn from(err: Box<bincode::ErrorKind>) -> Self {
        DatabaseError::BincodeError(err)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub path: String,
    pub create_if_missing: bool,
    pub backup_path: String,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        DatabaseConfig {
            path: String::from("./.rocksdb/"),
            create_if_missing: true,
            backup_path: String::from("./.rocksdb_backup/"),
        }
    }
}

pub struct DB {
    pub path: String,
    pub backup_path: String,
    pub db: Option<Arc<OptimisticTransactionDB>>,
}

impl DB {
    pub fn open_default(path: &str) -> Result<Self, DatabaseError> {
        let mut config = DatabaseConfig::default();
        config.path = path.to_string();
        DB::open(&config)
    }

    pub fn open(config: &DatabaseConfig) -> Result<Self, DatabaseError> {
        let mut opts = Options::default();
        opts.create_if_missing(config.create_if_missing);
        let db = OptimisticTransactionDB::open(&opts, &config.path)?;
        Ok(DB {
            path: config.path.to_owned(),
            backup_path: config.backup_path.to_owned(),
            db: Some(Arc::new(db)),
        })
    }

    pub fn destroy(path: &str, opts: Option<&Options>) -> Result<(), DatabaseError> {
        if let Some(o) = opts {
            rocksdb::DB::destroy(o, path)?;
        } else {
            rocksdb::DB::destroy(&Options::default(), path)?;
        }
        Ok(())
    }

    pub fn set_data(&self, key: &str, data: &[u8]) -> Result<(), DatabaseError> {
        if let Some(db) = &self.db {
            db.put(key.as_bytes(), data)?;
            Ok(())
        } else {
            Err(DatabaseError::DatabaseConnectionError)
        }
    }

    pub fn get_data(&self, key: &str) -> Result<Option<Vec<u8>>, DatabaseError> {
        if let Some(db) = &self.db {
            match db.get(key.as_bytes())? {
                Some(bytes) => Ok(Some(bytes)),
                None => Ok(None),
            }
        } else {
            Err(DatabaseError::DatabaseConnectionError)
        }
    }
    pub fn delete_data(&self, key: &str) -> Result<(), DatabaseError> {
        if let Some(db) = &self.db {
            db.delete(key.as_bytes())?;
            Ok(())
        } else {
            Err(DatabaseError::DatabaseConnectionError)
        }
    }

    pub fn multi_get_data(&self, keys: &[&str]) -> Result<Vec<Option<Vec<u8>>>, DatabaseError> {
        if let Some(db) = &self.db {
            let key_bytes: Vec<Vec<u8>> = keys.iter().map(|k| k.as_bytes().to_vec()).collect();
            let result = db
                .multi_get(key_bytes)
                .into_iter()
                .map(|r| match r {
                    Ok(v) => v,
                    Err(_) => None,
                })
                .collect();
            Ok(result)
        } else {
            Err(DatabaseError::DatabaseConnectionError)
        }
    }

    pub fn _put_timeseries(&self, key: &[u8], ts: &MetricTimeSeries) -> Result<(), DatabaseError> {
        // split timeseries into hourly buckets
        // Generate Vector of buckets and keys
        let mut ts_buckets: Vec<(Vec<u8>, MetricTimeSeries)> = Vec::new();
        for ts_bucket in ts.buckets() {
            if let Some(first_ts) = ts_bucket.first_timestamp() {
                // Generate key with timestamp
                let full_key = DB::_to_ts_key(key, first_ts);
                // Add key and bucket to vector
                ts_buckets.push((full_key, ts_bucket));
            }
        }
        // write batch to db
        self._upsert_timeseries_buckets(ts_buckets)
    }

    pub fn put_metric(
        &self,
        tenant_id: &TenantId,
        device_id: &str,
        metric_name: &str,
        value: MetricValue,
    ) -> Result<(), DatabaseError> {
        let key = format!("{}#{}#{}", tenant_id, device_id, metric_name).into_bytes();
        let generic_ts = value.as_timeseries(chrono::Utc::now().timestamp() as u64);
        self._put_timeseries(&key, &generic_ts)
    }

    // Upsert timeseries data into the database
    // the ts_buckets are tuples of key and timeseries data
    fn _upsert_timeseries_buckets(
        &self,
        ts_buckets: Vec<(Vec<u8>, MetricTimeSeries)>,
    ) -> Result<(), DatabaseError> {
        const MAX_RETRIES: u32 = 5;
        let mut retry_count = 0;

        while retry_count < MAX_RETRIES {
            if let Some(db) = &self.db {
                let txn = db.transaction();

                // Process each bucket
                for (key, new_ts) in &ts_buckets {
                    // Get existing bucket if it exists
                    let mut final_ts = match txn.get_for_update(key, false)? {
                        Some(data) => MetricTimeSeries::from_binary(&data)?,
                        None => MetricTimeSeries::new(),
                    };

                    // Merge new data
                    final_ts.merge(new_ts);

                    // Serialize and prepare write
                    let ts_data = final_ts.to_binary()?;
                    txn.put(key, &ts_data)?;
                }

                // Try to commit transaction
                match txn.commit() {
                    Ok(_) => return Ok(()),
                    Err(e) => {
                        retry_count += 1;
                        if retry_count < MAX_RETRIES {
                            continue;
                        } else {
                            return Err(DatabaseError::RocksDBError(e));
                        }
                    }
                }
            } else {
                return Err(DatabaseError::DatabaseConnectionError);
            }
        }

        Err(DatabaseError::DatabaseTransactionError(
            "Failed to commit timeseries update after max retries".to_string(),
        ))
    }

    // Return the timeseries data for a given key and timestamp range
    pub fn _get_timeseries(
        &self,
        key: &[u8],
        min_ts: u64,
        max_ts: u64,
    ) -> Result<MetricTimeSeries, DatabaseError> {
        let mut merged_ts = MetricTimeSeries::new();

        if let Some(db) = &self.db {
            let full_min_key = DB::_to_ts_key(key, min_ts);
            let full_max_key = DB::_to_ts_key(key, max_ts);

            // println!("Full min key: {:?}", String::from_utf8_lossy(&full_min_key).to_string());
            // println!("Full max key: {:?}", String::from_utf8_lossy(&full_max_key).to_string());

            let iter = db.iterator(rocksdb::IteratorMode::From(
                &full_max_key,
                rocksdb::Direction::Forward,
            ));

            for item in iter {
                match item {
                    Ok((key, value)) => {
                        if key > full_min_key.as_slice().into() {
                            break;
                        }
                        match MetricTimeSeries::from_binary(&value) {
                            Ok(ts) => {
                                merged_ts.merge(&ts);
                            }
                            Err(e) => return Err(DatabaseError::TimeseriesSerializationError(e)),
                        }
                    }
                    Err(e) => return Err(DatabaseError::RocksDBError(e)),
                }
            }
            merged_ts.trim(min_ts, max_ts);
            Ok(merged_ts)
        } else {
            Err(DatabaseError::DatabaseConnectionError)
        }
    }

    pub fn get_metric(
        &self,
        tenant_id: &TenantId,
        device_id: &str,
        metric_name: &str,
        start: u64,
        end: u64,
    ) -> Result<MetricTimeSeries, DatabaseError> {
        let key = format!("{}#{}#{}", tenant_id, device_id, metric_name).into_bytes();
        self._get_timeseries(&key, start, end)
    }

    pub fn get_last_metric(
        &self,
        tenant_id: &TenantId,
        device_id: &str,
        metric_name: &str,
        limit: u64,
    ) -> Result<MetricTimeSeries, DatabaseError> {
        let key = format!("{}#{}#{}", tenant_id, device_id, metric_name).into_bytes();
        self._get_timeseries_last(&key, limit)
    }

    // Return the last timeseries data for a given key
    pub fn _get_timeseries_last(
        &self,
        key_prefix: &[u8],
        limit: u64,
    ) -> Result<MetricTimeSeries, DatabaseError> {
        let mut merged_ts = MetricTimeSeries::new();
        let max_ts = chrono::Utc::now().timestamp() as u64 + MAX_FUTURE_SECONDS;

        if let Some(db) = &self.db {
            let full_max_key = DB::_to_ts_key(key_prefix, max_ts);
            let iter = db.iterator(rocksdb::IteratorMode::From(
                &full_max_key,
                rocksdb::Direction::Forward,
            ));

            let mut count: u64 = 0;
            for item in iter {
                match item {
                    Ok((key, value)) => {
                        // check if key is still within the prefix (starts with key_prefix)
                        if !key.starts_with(key_prefix) {
                            break;
                        }

                        match MetricTimeSeries::from_binary(&value) {
                            Ok(ts) => {
                                merged_ts.merge(&ts);
                                count += merged_ts.len() as u64;
                            }
                            Err(e) => return Err(DatabaseError::TimeseriesSerializationError(e)),
                        }
                        if count >= limit {
                            break;
                        }
                    }
                    Err(e) => return Err(DatabaseError::RocksDBError(e)),
                }
            }

            // Trim to limit
            merged_ts.keep_last(limit as usize);
            Ok(merged_ts)
        } else {
            Err(DatabaseError::DatabaseConnectionError)
        }
    }

    pub fn _to_ts_key(key: &[u8], ts: u64) -> Vec<u8> {
        let ts_key = MetricTimeSeries::ts_to_key(ts);
        [key, b"#", ts_key.as_bytes()].concat()
    }

    pub fn _from_ts_key(full_key: &[u8]) -> Result<(&[u8], u64), DatabaseError> {
        // Find separator position
        let sep_pos =
            full_key
                .windows(1)
                .position(|w| w == b"#")
                .ok_or(DatabaseError::InvalidKeyError(
                    String::from_utf8_lossy(full_key).to_string(),
                ))?;

        // Split key into parts
        let (key, ts_key) = full_key.split_at(sep_pos);

        // Skip separator and convert timestamp
        let ts = MetricTimeSeries::key_to_ts(std::str::from_utf8(&ts_key[1..]).map_err(|_| {
            DatabaseError::InvalidKeyError(String::from_utf8_lossy(full_key).to_string())
        })?)
        .map_err(|_| {
            DatabaseError::InvalidKeyError(String::from_utf8_lossy(full_key).to_string())
        })?;

        Ok((key, ts))
    }

    fn _to_shadow_key(device_id: &str, shadow_name: &ShadowName, tenant_id: &TenantId) -> Vec<u8> {
        format!("{}#{}#{}", tenant_id, device_id, shadow_name.as_str()).into_bytes()
    }

    pub fn _upsert_shadow(&self, update: &StateUpdateDocument) -> Result<Shadow, DatabaseError> {
        const MAX_RETRIES: u32 = 5;
        let mut retry_count = 0;
        let key = Self::_to_shadow_key(&update.device_id, &update.shadow_name, &update.tenant_id);

        while retry_count < MAX_RETRIES {
            if let Some(db) = &self.db {
                let txn = db.transaction();

                // Get existing shadow or create new
                let mut shadow = match txn.get_for_update(&key, false)? {
                    Some(data) => {
                        let shadow_str = String::from_utf8(data).map_err(|_e| {
                            DatabaseError::DatabaseValueError("Invalid UTF-8".to_string())
                        })?;
                        Shadow::from_json(&shadow_str)?
                    }
                    None => Shadow::new(&update.device_id, &update.shadow_name, &update.tenant_id),
                };

                // Apply update
                shadow.update(update)?;

                // Serialize and write
                let shadow_data = shadow.to_json()?.into_bytes();

                txn.put(&key, &shadow_data)?;

                match txn.commit() {
                    Ok(_) => return Ok(shadow),
                    Err(e) => {
                        retry_count += 1;
                        if retry_count < MAX_RETRIES {
                            continue;
                        } else {
                            return Err(DatabaseError::RocksDBError(e));
                        }
                    }
                }
            } else {
                return Err(DatabaseError::DatabaseConnectionError);
            }
        }

        Err(DatabaseError::DatabaseTransactionError(
            "Failed to commit shadow update after max retries".to_string(),
        ))
    }

    pub fn _get_shadow(
        &self,
        device_id: &str,
        shadow_name: &ShadowName,
        tenant_id: &TenantId,
    ) -> Result<Shadow, DatabaseError> {
        if let Some(db) = &self.db {
            let key = Self::_to_shadow_key(device_id, shadow_name, tenant_id);

            match db.get(&key)? {
                Some(data) => {
                    let shadow_str = String::from_utf8(data).map_err(|_e| {
                        DatabaseError::DatabaseValueError("Invalid UTF-8".to_string())
                    })?;
                    Ok(Shadow::from_json(&shadow_str)?)
                }
                None => Err(DatabaseError::NotFoundError(format!(
                    "Shadow not found for device = {} name = {} tenant = {}",
                    device_id, shadow_name, tenant_id
                ))),
            }
        } else {
            Err(DatabaseError::DatabaseConnectionError)
        }
    }

    pub fn flush(&self) -> Result<(), DatabaseError> {
        if let Some(db) = &self.db {
            db.flush()?;
            Ok(())
        } else {
            Err(DatabaseError::DatabaseConnectionError)
        }
    }

    pub fn cancel_all_background_tasks(&self, wait: Option<bool>) -> Result<(), DatabaseError> {
        let wait_flag = wait.unwrap_or(false);
        if let Some(db) = &self.db {
            db.cancel_all_background_work(wait_flag);
            Ok(())
        } else {
            Err(DatabaseError::DatabaseConnectionError)
        }
    }

    fn _to_dataconfig_key(tenant_id: &TenantId, device_id_prefix: Option<&str>) -> Vec<u8> {
        match device_id_prefix {
            Some(did) => format!("dc#{}#{}", tenant_id, did).into_bytes(),
            None => format!("dc#{}", tenant_id).into_bytes(),
        }
    }

    pub fn store_tenant_data_config(
        &self,
        tenant_id: &TenantId,
        config: &DataConfig,
    ) -> Result<(), DatabaseError> {
        let key = Self::_to_dataconfig_key(tenant_id, None);
        let data = config.to_json().into_bytes();
        self.set_data(&String::from_utf8_lossy(&key), &data)
    }

    pub fn store_device_data_config(
        &self,
        tenant_id: &TenantId,
        device_id_prefix: &str,
        config: &DataConfig,
    ) -> Result<(), DatabaseError> {
        let key = Self::_to_dataconfig_key(tenant_id, Some(device_id_prefix));
        let data = config.to_json().into_bytes();
        self.set_data(&String::from_utf8_lossy(&key), &data)
    }

    pub fn get_data_config(
        &self,
        tenant_id: &TenantId,
        device_id: Option<&str>,
    ) -> Result<Option<DataConfig>, DatabaseError> {
        // Get tenant config first
        let tenant_key = Self::_to_dataconfig_key(tenant_id, None);
        let tenant_key_str = String::from_utf8_lossy(&tenant_key);
        let maybe_tenant_cfg = match self.get_data(&tenant_key_str)? {
            Some(bytes) => Some(DataConfig::from_json(&String::from_utf8_lossy(&bytes))),
            None => None,
        };

        // If no device_id specified, return tenant config
        if device_id.is_none() {
            return Ok(maybe_tenant_cfg);
        }

        // Search for device config using prefix
        if let Some(db) = &self.db {
            let search_key = Self::_to_dataconfig_key(tenant_id, device_id);
            let mut iter = db.iterator(rocksdb::IteratorMode::From(
                &search_key,
                rocksdb::Direction::Reverse,
            ));

            // Look for longest matching prefix
            while let Some(Ok((key, value))) = iter.next() {
                let key_str = String::from_utf8_lossy(&key);
                if key_str.starts_with(tenant_key_str.as_ref()) {
                    let device_cfg = DataConfig::from_json(&String::from_utf8_lossy(&value));
                    // if we have a tenant config, merge with device config
                    if let Some(tenant_cfg) = maybe_tenant_cfg {
                        return Ok(Some(tenant_cfg.merge_with(&device_cfg)));
                    } else {
                        return Ok(Some(device_cfg));
                    }
                }
            }
        }

        // No matching device config found
        Ok(maybe_tenant_cfg)
    }

    pub fn delete_data_config(
        &self,
        tenant_id: &TenantId,
        device_id_prefix: Option<&str>,
    ) -> Result<(), DatabaseError> {
        let key = Self::_to_dataconfig_key(tenant_id, device_id_prefix);
        self.delete_data(&String::from_utf8_lossy(&key))
    }

    pub fn list_data_configs(
        &self,
        tenant_id: &TenantId,
    ) -> Result<Vec<DataConfigEntry>, DatabaseError> {
        let mut configs = Vec::new();
        let tenant_prefix = format!("dc#{}", tenant_id);

        if let Some(db) = &self.db {
            let iter = db.iterator(rocksdb::IteratorMode::From(
                tenant_prefix.as_bytes(),
                rocksdb::Direction::Forward,
            ));

            for item in iter {
                match item {
                    Ok((key, value)) => {
                        let key_str = String::from_utf8_lossy(&key);
                        if !key_str.starts_with(&tenant_prefix) {
                            break;
                        }
                        let config = DataConfig::from_json(&String::from_utf8_lossy(&value));
                        // split key_str into tenant_id and device_prefix (seperated by #)
                        let parts: Vec<&str> = key_str.split('#').collect();
                        let device_prefix = {
                            if parts.len() > 2 {
                                Some(parts[2].to_string())
                            } else {
                                None
                            }
                        };
                        configs.push(DataConfigEntry {
                            tenant_id: tenant_id.to_owned(),
                            device_prefix: device_prefix,
                            metrics: config.metrics,
                        });
                    }
                    Err(e) => return Err(DatabaseError::RocksDBError(e)),
                }
            }
        } else {
            return Err(DatabaseError::DatabaseConnectionError);
        }

        Ok(configs)
    }

    pub fn create_backup(&self) -> Result<String, DatabaseError> {
        let backup_path = self.backup_path.clone();
        backup_db(&self, &backup_path)
    }

    fn _to_device_metadata_key(tenant_id: &TenantId, device_id: &str) -> Vec<u8> {
        format!("device#{}#{}", tenant_id, device_id).into_bytes()
    }

    pub fn put_device_metadata(&self, metadata: &DeviceMetadata) -> Result<(), DatabaseError> {
        if let Some(db) = &self.db {
            let key = Self::_to_device_metadata_key(&metadata.tenant_id, &metadata.device_id);
            let data = serde_json::to_vec(metadata).map_err(|e| {
                DatabaseError::DatabaseValueError(format!("Failed to serialize device metadata: {}", e))
            })?;
            db.put(key, data)?;
            Ok(())
        } else {
            Err(DatabaseError::DatabaseConnectionError)
        }
    }

    pub fn get_device_metadata(
        &self,
        tenant_id: &TenantId,
        device_id: &str,
    ) -> Result<Option<DeviceMetadata>, DatabaseError> {
        if let Some(db) = &self.db {
            let key = Self::_to_device_metadata_key(tenant_id, device_id);
            match db.get(&key)? {
                Some(data) => {
                    let metadata = serde_json::from_slice(&data).map_err(|e| {
                        DatabaseError::DatabaseValueError(format!("Failed to deserialize device metadata: {}", e))
                    })?;
                    Ok(Some(metadata))
                }
                None => Ok(None),
            }
        } else {
            Err(DatabaseError::DatabaseConnectionError)
        }
    }

    pub fn list_devices(&self, tenant_id: &TenantId) -> Result<Vec<DeviceMetadata>, DatabaseError> {
        let mut devices = Vec::new();
        let prefix = format!("device#{}", tenant_id);

        if let Some(db) = &self.db {
            let iter = db.iterator(rocksdb::IteratorMode::From(
                prefix.as_bytes(),
                rocksdb::Direction::Forward,
            ));

            for item in iter {
                match item {
                    Ok((key, value)) => {
                        let key_str = String::from_utf8_lossy(&key);
                        // Stop iteration when we reach keys that don't match our prefix
                        if !key_str.starts_with(&prefix) {
                            break;
                        }

                        match serde_json::from_slice(&value) {
                            Ok(metadata) => devices.push(metadata),
                            Err(e) => {
                                return Err(DatabaseError::DatabaseValueError(format!(
                                    "Failed to deserialize device metadata: {}",
                                    e
                                )))
                            }
                        }
                    }
                    Err(e) => return Err(DatabaseError::RocksDBError(e)),
                }
            }
        } else {
            return Err(DatabaseError::DatabaseConnectionError);
        }

        Ok(devices)
    }

    pub fn delete_device_metadata(
        &self,
        tenant_id: &TenantId,
        device_id: &str,
    ) -> Result<(), DatabaseError> {
        let key = Self::_to_device_metadata_key(tenant_id, device_id);
        self.delete_data(&String::from_utf8_lossy(&key))
    }
}


fn backup_db(db: &DB, backup_path: &str) -> Result<String, DatabaseError> {
    let backup_dir = Path::new(backup_path);
    let backup_opts = BackupEngineOptions::new(backup_dir)?;
    let backup_env = Env::new()?;
    let mut backup_engine = BackupEngine::open(&backup_opts, &backup_env)?;
    let inner_db = db.db.as_ref().ok_or(DatabaseError::DatabaseConnectionError)?;
    warn!("Creating backup");
    let _ = backup_engine.create_new_backup_flush(inner_db, true)?;

    // Cleanup old backups
    let _ = backup_engine.purge_old_backups(3)?;

    // Get Buckup Info
    let mut last_id = 0;
    let mut last_timestamp = 0;
    let mut last_size = 0;
    let backup_info = backup_engine.get_backup_info();
    for info in backup_info {
        warn!("Backup: {}, {}, {}", info.backup_id, info.timestamp, info.size);
        last_id = info.backup_id;
        last_timestamp = info.timestamp;
        last_size = info.size;
    }

    Ok(format!(
        "Backup created at {} with id: {} timestamp: {} size: {}",
        backup_path, last_id, last_timestamp, last_size
    ))
}


#[cfg(test)]
mod tests;
