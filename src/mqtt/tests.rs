use super::*;
use std::time::Duration;
use tokio::time::sleep;

fn get_test_config() -> Option<MqttConfig> {
    let mut config = MqttConfig::default();
    config.bind_v3 = "127.0.0.1:7777".to_string();
    config.bind_v5 = "127.0.0.1:7778".to_string();
    Some(config)
}

#[ignore]
#[tokio::test]
async fn test_server_start_stop() {
    let config = get_test_config();
    let mut server = start_broker(config).await;

    let shutdown_received = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let shutdown_received_clone = shutdown_received.clone();
    let cancel_token = server.get_cancel_token();

    tokio::spawn(async move {
        cancel_token.cancelled().await;
        shutdown_received_clone.store(true, std::sync::atomic::Ordering::SeqCst);
    });

    sleep(Duration::from_millis(100)).await;
    server.shutdown();

    sleep(Duration::from_millis(100)).await;
    assert!(shutdown_received.load(std::sync::atomic::Ordering::SeqCst));
}

#[ignore]
#[tokio::test]
async fn test_publish_subscribe() {
    let config = get_test_config();
    let mut server = start_broker(config).await;

    // Create receiver
    let receiver = server.message_receiver();

    // Subscribe to topic
    server
        .mqtt
        .subscribe("test/topic".to_string())
        .await
        .unwrap();
    sleep(Duration::from_millis(100)).await;

    // Publish message
    let test_payload = b"test message".to_vec();
    server
        .mqtt
        .publish("test/topic".to_string(), test_payload.clone())
        .unwrap();

    // Receive message
    if let Ok(msg) = receiver.recv() {
        assert_eq!(msg.topic, "test/topic");
        assert_eq!(msg.payload, test_payload);
    }

    server.shutdown();
}
