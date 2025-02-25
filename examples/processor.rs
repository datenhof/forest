extern crate forest;

use forest::{config::ForestConfig, server::start_server};
use tracing::Level;

#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() {
    let builder = tracing_subscriber::fmt()
        .with_line_number(false)
        .with_file(false)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_max_level(Level::INFO);

    builder
        .try_init()
        .expect("initialized subscriber succesfully");

    let config = ForestConfig::default();

    let cancel_token = start_server(&config).await;

    tokio::select! {
        _ = cancel_token.cancelled() => {
            tracing::warn!("Server cancelled");
            return;
        },
        _ = tokio::signal::ctrl_c() => {},
    };
}
