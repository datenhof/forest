use crate::api::handlers::*;
use crate::api::AppState;
use axum::{routing::get, routing::put, Router};

pub fn get_routes(state: AppState) -> Router {
    Router::new()
        .route("/", get(home_handler))
        .route("/shadow/{device_id}", get(get_shadow_handler))
        .route("/shadow/{device_id}", put(update_shadow_handler))
        .route("/data/{device_id}/{metric}", get(get_timeseries_handler))
        .route(
            "/data/{device_id}/{metric}/last",
            get(get_last_timeseries_handler),
        )
        .route(
            "/dataconfig/{tenant_id}",
            put(store_tenant_config_handler)
                .get(get_config_handler)
                .delete(delete_config_handler),
        )
        .route(
            "/dataconfig/{tenant_id}/device/{device_prefix}",
            put(store_device_config_handler)
                .get(get_config_handler)
                .delete(delete_config_handler),
        )
        .route("/dataconfig/{tenant_id}/all", get(list_configs_handler))
        .route("/connected/{tenant_id}", get(list_connections_handler))
        .route("/database/backup", get(backup_database_handler))
        .with_state(state)
}
