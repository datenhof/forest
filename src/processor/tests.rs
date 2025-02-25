use super::*;
use crate::db::DB;
use crate::mqtt::{start_broker, MqttServer};
use tempfile::TempDir;

fn setup_db() -> Arc<DB> {
    let temp_dir = TempDir::new().unwrap();
    let mut options = rocksdb::Options::default();
    options.create_if_missing(true);
    Arc::new(DB::open_default(temp_dir.path().to_str().unwrap()).unwrap())
}

async fn setup_mqtt() -> MqttServer {
    start_broker(None).await
}

#[tokio::test]
async fn test_start_processor() {
    let db = setup_db();
    let mut mqtt = setup_mqtt().await;
    let sender = mqtt.mqtt.clone();
    let receiver = mqtt.message_receiver();
    let conn_mon_rx = mqtt.connection_monitor_subscribe();
    let connected_clients = Arc::new(ConnectionSet::new());
    let processor_config = ProcessorConfig::default();
    let result = start_processor(
        db,
        sender,
        receiver,
        conn_mon_rx,
        connected_clients,
        processor_config,
    )
    .await;
    assert!(result.is_ok(), "start_processor should return Ok");
    let processor = result.unwrap();
    assert!(processor.db.db.is_some(), "DB should be open");
}
