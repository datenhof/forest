use crate::api::handlers::*;
use crate::api::AppState;
use axum::{routing::get, routing::put, Router};

pub fn get_routes(state: AppState) -> Router {
    Router::new()
        .route("/", get(home_handler))
        .route("/health", get(health_handler))
        .route("/{tenant_id}/shadow/{device_id}", get(get_shadow_handler))
        .route("/{tenant_id}/shadow/{device_id}", put(update_shadow_handler))
        // .route("/{tenant_id}/shadow/{device_id}/{shadow_name}", get(get_named_shadow_handler))
        // .route("/{tenant_id}/shadow/{device_id}/{shadow_name}", put(update_named_shadow_handler))
        .route("/{tenant_id}/data/{device_id}/{metric}", get(get_timeseries_handler))
        .route(
            "/{tenant_id}/data/{device_id}/{metric}/last",
            get(get_last_timeseries_handler),
        )
        .route(
            "/{tenant_id}/dataconfig",
            put(store_tenant_config_handler)
                .get(get_tenant_config_handler)
                .delete(delete_config_handler),
        )
        .route(
            "/{tenant_id}/dataconfig/device/{device_prefix}",
            put(store_device_config_handler)
                .get(get_config_handler)
                .delete(delete_config_handler),
        )
        .route("/{tenant_id}/dataconfig/all", get(list_configs_handler))
        .route("/{tenant_id}/connected", get(list_connections_handler))
        .route(
            "/{tenant_id}/devices",
            get(list_devices_handler)
        )
        .route(
            "/{tenant_id}/devices/{device_id}",
            get(get_device_info_handler)
                .post(post_device_metadata_handler)
                .delete(delete_device_metadata_handler)
        )
        .route(
            "/{tenant_id}/devices/{device_id}/metadata",
            get(get_device_metadata_handler)
        )
        .route("/database/backup", get(backup_database_handler))
        .with_state(state)
}
