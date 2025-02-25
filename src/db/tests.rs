use super::*;
use crate::dataconfig::{DataConfig, DataType, MetricConfig};
use crate::shadow::StateDocument;
use crate::timeseries::FloatTimeSeries;
use serde_json::{json, Value};
use tempfile::TempDir;

fn setup_db() -> (DB, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let mut config = DatabaseConfig::default();
    config.path = temp_dir.path().to_str().unwrap().to_string();
    let backup_path = temp_dir.path().join("backup");
    config.backup_path = backup_path.to_str().unwrap().to_string();

    let db = DB::open(&config).unwrap();
    // let mut options = rocksdb::Options::default();
    // options.create_if_missing(true);
    // let db = Arc::new(OptimisticTransactionDB::open(&options, temp_dir.path()).unwrap());
    // let db_py = DB {
    //     path: temp_dir.path().to_str().unwrap().to_string(),
    //     backup_path
    //     db: Some(db),
    // };
    (db, temp_dir)
}

#[test]
fn test_put_get_multiple_buckets() {
    let (db, _temp) = setup_db();
    let mut ts = FloatTimeSeries::new();
    // Two hours of data
    ts.add_point(1710511200, 1.0); // Hour 1
    ts.add_point(1710511200 + 3600, 2.0); // Hour 2

    let key = b"test2";
    db._put_timeseries(key, &MetricTimeSeries::from(&ts))
        .unwrap();

    // Query first hour only
    let result = db
        ._get_timeseries(key, 1710511200, 1710511200 + 3599)
        .unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(
        *result.get_value_for_timestamp(1710511200).unwrap(),
        MetricValue::Float(1.0)
    );
}

#[test]
fn test_no_db_connection_ts() {
    let db = DB {
        path: String::from("path"),
        backup_path: String::from("backup"),
        db: None,
    };
    let ts = FloatTimeSeries::new();

    assert!(matches!(
        db._put_timeseries(b"key", &MetricTimeSeries::from(&ts)),
        Err(DatabaseError::DatabaseConnectionError)
    ));

    assert!(matches!(
        db._get_timeseries(b"key", 0, 1),
        Err(DatabaseError::DatabaseConnectionError)
    ));
}

#[test]
fn test_empty_range() {
    let (db, _temp) = setup_db();
    let result = db._get_timeseries(b"nonexistent", 0, 1).unwrap();
    assert_eq!(result.len(), 0);
}

#[test]
fn test_ts_key_ordering() {
    // Base keys
    let key_a = b"a";
    let key_b = b"b";
    let key_ab = b"ab";

    // March 15, 2024 14:00-16:00 UTC
    let ts1 = 1710511200; // 14:00
    let ts2 = ts1 + 3600; // 15:00
    let ts3 = ts1 + 7200; // 16:00

    // Generate full keys
    let key_a1 = DB::_to_ts_key(key_a, ts1);
    let key_a2 = DB::_to_ts_key(key_a, ts2);
    let key_a3 = DB::_to_ts_key(key_a, ts3);
    let key_b1 = DB::_to_ts_key(key_b, ts1);
    let key_ab1 = DB::_to_ts_key(key_ab, ts2);

    // Convert to strings for inspection
    let str_a1 = String::from_utf8_lossy(&key_a1);
    let str_a2 = String::from_utf8_lossy(&key_a2);
    let str_a3 = String::from_utf8_lossy(&key_a3);
    let str_b1 = String::from_utf8_lossy(&key_b1);
    let str_ab1 = String::from_utf8_lossy(&key_ab1);

    // Verify string format
    assert_eq!(str_a1, "a#0976091609");
    assert_eq!(str_a2, "a#0976091608");
    assert_eq!(str_a3, "a#0976091607");
    assert_eq!(str_ab1, "ab#0976091608");
    assert_eq!(str_b1, "b#0976091609");

    // Test ordering within same base key
    assert!(key_a1 > key_a2);
    assert!(key_a2 > key_a3);

    // Test ordering across different base keys
    assert!(key_a1 < key_b1);
    assert!(key_a2 < key_b1);
    assert!(key_a3 < key_b1);

    // Test ordering with different base key lengths
    assert!(key_a1 < key_ab1);
    assert!(key_a2 < key_ab1);
    assert!(key_a3 < key_ab1);
    assert!(key_b1 > key_ab1);

    // Test key parsing
    let (parsed_key, parsed_ts) = DB::_from_ts_key(&key_a1).unwrap();
    assert_eq!(parsed_key, key_a);
    assert_eq!(parsed_ts, ts1);
}

#[test]
fn test_get_timeseries_last() {
    let (db, _temp) = setup_db();

    // Create test data with multiple timestamps
    let mut ts1 = FloatTimeSeries::new();
    ts1.add_point(1710511200, 1.0); // 14:00
    ts1.add_point(1710511200 + 1800, 2.0); // 14:30

    let mut ts2 = FloatTimeSeries::new();
    ts2.add_point(1710514800, 3.0); // 15:00
    ts2.add_point(1710514800 + 1800, 4.0); // 15:30

    let key = b"test_last";

    // Store both timeseries
    db._put_timeseries(key, &MetricTimeSeries::from(&ts1))
        .unwrap();
    db._put_timeseries(key, &MetricTimeSeries::from(&ts2))
        .unwrap();

    // Test getting last point
    let result = db._get_timeseries_last(key, 1).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(
        *result.get_value_for_timestamp(1710514800 + 1800).unwrap(),
        MetricValue::Float(4.0)
    );

    // Test getting more points than available
    let result = db._get_timeseries_last(key, 10).unwrap();
    assert_eq!(result.len(), 4);

    // Test empty key
    let result = db._get_timeseries_last(b"nonexistent", 1).unwrap();
    assert_eq!(result.len(), 0);

    // Test no DB connection
    let db_no_conn = DB {
        path: "path".to_string(),
        backup_path: "backup".to_string(),
        db: None,
    };
    assert!(matches!(
        db_no_conn._get_timeseries_last(key, 1),
        Err(DatabaseError::DatabaseConnectionError)
    ));
}

#[test]
fn test_upsert_shadow() {
    let (db, _temp) = setup_db();

    // Create initial update
    let update1 = StateUpdateDocument {
        device_id: "thermostat-01".to_string(),
        shadow_name: ShadowName::Default,
        tenant_id: TenantId::Default,
        state: StateDocument {
            reported: json!({
                "temperature": 22.5,
                "humidity": 45
            }),
            desired: Value::Null,
            delta: Value::Null,
        },
    };

    // Test initial insert
    db._upsert_shadow(&update1).unwrap();

    // Verify shadow was created
    let key = DB::_to_shadow_key(&update1.device_id, &update1.shadow_name, &update1.tenant_id);
    let shadow_data = db.db.as_ref().unwrap().get(&key).unwrap().unwrap();
    let shadow: Shadow = serde_json::from_slice(&shadow_data).unwrap();

    assert_eq!(shadow.device_id, "thermostat-01");
    assert_eq!(shadow.get_reported_value()["temperature"], 22.5);

    // Test update existing shadow
    let update2 = StateUpdateDocument {
        device_id: "thermostat-01".to_string(),
        shadow_name: ShadowName::Default,
        tenant_id: TenantId::Default,
        state: StateDocument {
            reported: Value::Null,
            desired: json!({
                "temperature": 21.0,
            }),
            delta: Value::Null,
        },
    };

    db._upsert_shadow(&update2).unwrap();

    // Verify shadow was updated
    let shadow_data = db.db.as_ref().unwrap().get(&key).unwrap().unwrap();
    let shadow: Shadow = serde_json::from_slice(&shadow_data).unwrap();
    let desired = shadow.get_desired_value();
    let reported = shadow.get_reported_value();
    assert_eq!(desired["temperature"], 21.0);
    assert_eq!(reported["temperature"], 22.5);
    let s = shadow.get_delta_value();
    assert_eq!(s["temperature"], 21.0);

    // Make another update to reset delta
    let update3 = StateUpdateDocument {
        device_id: "thermostat-01".to_string(),
        shadow_name: ShadowName::Default,
        tenant_id: TenantId::Default,
        state: StateDocument {
            reported: json!({
                "temperature": 21.0
            }),
            desired: Value::Null,
            delta: Value::Null,
        },
    };
    db._upsert_shadow(&update3).unwrap();

    let shadow_data = db.db.as_ref().unwrap().get(&key).unwrap().unwrap();
    let shadow: Shadow = serde_json::from_slice(&shadow_data).unwrap();
    let desired = shadow.get_desired_value();
    let reported = shadow.get_reported_value();
    assert_eq!(desired["temperature"], 21.0);
    assert_eq!(reported["temperature"], 21.0);
    assert_eq!(*shadow.get_delta_value(), Value::Null);

    let store_shadow = db
        ._get_shadow("thermostat-01", &ShadowName::Default, &TenantId::Default)
        .unwrap();
    let desired = shadow.get_desired_value();
    let reported = shadow.get_reported_value();
    assert_eq!(store_shadow.device_id, "thermostat-01");
    assert_eq!(reported["temperature"], 21.0);
    assert_eq!(desired["temperature"], 21.0);
    assert_eq!(*store_shadow.get_delta_value(), Value::Null);
}

#[test]
fn test_store_and_get_tenant_data_config() {
    let (db, _temp) = setup_db();

    let config = DataConfig {
        metrics: vec![
            MetricConfig {
                json_pointer: "/temperature".to_string(),
                name: "temperature".to_string(),
                data_type: DataType::Float,
            },
            MetricConfig {
                json_pointer: "/temperature".to_string(),
                name: "humidity".to_string(),
                data_type: DataType::Int,
            },
        ],
    };

    db.store_tenant_data_config(&TenantId::Default, &config).unwrap();
    let actual = db.get_data_config(&TenantId::Default, None).unwrap().unwrap();
    assert_eq!(actual.metrics.len(), 2);
}

#[test]
fn test_store_and_get_device_data_config() {
    let (db, _temp) = setup_db();

    let tenant_config = DataConfig {
        metrics: vec![MetricConfig {
            json_pointer: "/temperature".to_string(),
            name: "temperature".to_string(),
            data_type: DataType::Float,
        }],
    };
    db.store_tenant_data_config(&TenantId::new("tenant2"), &tenant_config)
        .unwrap();

    let base = db
        .get_data_config(&TenantId::new("tenant2"), Some("deviceA1"))
        .unwrap()
        .unwrap();
    assert_eq!(base.metrics.len(), 1);
    assert_eq!(base.metrics[0].name, "temperature");
    assert_eq!(base.metrics[0].data_type, DataType::Float);

    let device_config = DataConfig {
        metrics: vec![MetricConfig {
            json_pointer: "/temperature".to_string(),
            name: "temperature".to_string(),
            data_type: DataType::Int, // override
        }],
    };
    db.store_device_data_config(&TenantId::new("tenant2"), "deviceA", &device_config)
        .unwrap();

    // Should merge tenant config + device config
    let merged = db
        .get_data_config(&TenantId::new("tenant2"), Some("deviceA1"))
        .unwrap()
        .unwrap();
    assert_eq!(merged.metrics.len(), 1);
    assert_eq!(merged.metrics[0].name, "temperature");
    assert_eq!(merged.metrics[0].data_type, DataType::Int);

    let device_config = DataConfig {
        metrics: vec![MetricConfig {
            json_pointer: "/temp3".to_string(),
            name: "temp2".to_string(),
            data_type: DataType::Float,
        }],
    };
    db.store_device_data_config(&TenantId::new("tenant2"), "deviceA1", &device_config)
        .unwrap();

    let merged = db
        .get_data_config(&TenantId::new("tenant2"), Some("deviceA1"))
        .unwrap()
        .unwrap();
    assert_eq!(merged.metrics.len(), 2);
    assert_eq!(merged.metrics[0].name, "temperature");
    assert_eq!(merged.metrics[0].data_type, DataType::Float);
    assert_eq!(merged.metrics[1].name, "temp2");
    assert_eq!(merged.metrics[1].data_type, DataType::Float);
}

#[test]
fn test_delete_data_config() {
    let (db, _temp) = setup_db();

    // Setup test data
    let tenant_config = DataConfig {
        metrics: vec![MetricConfig {
            json_pointer: "/temperature".to_string(),
            name: "temperature".to_string(),
            data_type: DataType::Float,
        }],
    };
    let device_config = DataConfig {
        metrics: vec![MetricConfig {
            json_pointer: "/humidity".to_string(),
            name: "humidity".to_string(),
            data_type: DataType::Int,
        }],
    };

    // Store configs
    db.store_tenant_data_config(&TenantId::new("tenant1"), &tenant_config)
        .unwrap();
    db.store_device_data_config(&TenantId::new("tenant1"), "device1", &device_config)
        .unwrap();

    // Delete device config
    db.delete_data_config(&TenantId::new("tenant1"), Some("device1")).unwrap();

    // Verify device config is gone but tenant config remains
    let result = db
        .get_data_config(&TenantId::new("tenant1"), Some("device1"))
        .unwrap()
        .unwrap();
    assert_eq!(result.metrics.len(), 1);
    assert_eq!(result.metrics[0].name, "temperature");

    // Delete tenant config
    db.delete_data_config(&TenantId::new("tenant1"), None).unwrap();

    // Verify tenant config is gone
    let result = db.get_data_config(&TenantId::new("tenant1"), None).unwrap();
    assert!(matches!(result, None));
}

#[test]
fn test_list_data_configs() {
    let (db, _temp) = setup_db();

    // Setup test data
    let tenant_config = DataConfig {
        metrics: vec![MetricConfig {
            json_pointer: "/temperature".to_string(),
            name: "temperature".to_string(),
            data_type: DataType::Float,
        }],
    };
    let device1_config = DataConfig {
        metrics: vec![MetricConfig {
            json_pointer: "/humidity".to_string(),
            name: "humidity".to_string(),
            data_type: DataType::Int,
        }],
    };
    let device2_config = DataConfig {
        metrics: vec![MetricConfig {
            json_pointer: "/pressure".to_string(),
            name: "pressure".to_string(),
            data_type: DataType::Float,
        }],
    };

    // Store configs
    db.store_tenant_data_config(&TenantId::new("tenant1"), &tenant_config)
        .unwrap();
    db.store_device_data_config(&TenantId::new("tenant1"), "device1", &device1_config)
        .unwrap();
    db.store_device_data_config(&TenantId::new("tenant1"), "device2", &device2_config)
        .unwrap();

    // List configs
    let configs = db.list_data_configs(&TenantId::new("tenant1")).unwrap();

    // Verify number of configs
    assert_eq!(configs.len(), 3);

    // Verify tenant config
    let tenant_key = TenantId::new("tenant1");
    let tenant_entry = configs
        .iter()
        .find(|entry| entry.tenant_id == tenant_key)
        .unwrap();
    assert_eq!(tenant_entry.metrics[0].name, "temperature");

    // Verify device configs
    let device1_key = Some("device1".to_string());
    let device1_entry = configs
        .iter()
        .find(|entry| entry.device_prefix == device1_key)
        .unwrap();
    assert_eq!(device1_entry.metrics[0].name, "humidity");

    let device2_key = Some("device2".to_string());
    let device2_entry = configs
        .iter()
        .find(|entry| entry.device_prefix == device2_key)
        .unwrap();
    assert_eq!(device2_entry.metrics[0].name, "pressure");

    // Verify empty list for non-existent tenant
    let empty_configs = db.list_data_configs(&TenantId::new("tenant2")).unwrap();
    assert_eq!(empty_configs.len(), 0);
}
