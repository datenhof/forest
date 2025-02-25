pub mod error;
pub mod handlers;
pub mod routes;
pub mod client;

use tokio_util::sync::CancellationToken;

use crate::api::routes::get_routes;
use crate::config::ForestConfig;
use crate::db::DB;
use crate::mqtt::{MqttSender, MqttServerMetrics};
use crate::server::ConnectionSet;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    db: Arc<DB>,
    mqtt_sender: Option<MqttSender>,
    mqtt_metrics: Arc<MqttServerMetrics>,
    connected_clients: Arc<ConnectionSet>,
    shadow_topic_prefix: String,
}

pub async fn start_api_server(
    bind_addr: &str,
    db: Arc<DB>,
    mqtt_sender: Option<MqttSender>,
    mqtt_metrics: Arc<MqttServerMetrics>,
    connected_clients: Arc<ConnectionSet>,
    config: &ForestConfig,
) -> CancellationToken {
    let state = AppState {
        db: db.clone(),
        mqtt_sender,
        mqtt_metrics,
        connected_clients,
        shadow_topic_prefix: config.processor.shadow_topic_prefix.to_owned(),
    };
    let app = get_routes(state);
    let listener = tokio::net::TcpListener::bind(bind_addr).await.unwrap();
    let cancel_token = CancellationToken::new();

    let server_cancel_token = cancel_token.clone();
    let _server_handle = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                _ = server_cancel_token.cancelled().await;
            })
            .await
            .unwrap();
    });

    cancel_token
}
