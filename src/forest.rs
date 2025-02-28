extern crate forest;

use std::path::PathBuf;
use std::sync::Arc;

use forest::config::ForestConfig;
use forest::db::DB;
use forest::models::TenantId;
use forest::server::start_server;
use forest::cli::{Cli, Commands};
use forest::api::client::create_backup;
use forest::api::services::create_device as create_device_api;
use forest::certs::CertificateManager;
use tokio::runtime::Runtime;
use tracing::Level;
use clap::Parser;

fn main() {
    let cli = Cli::parse();

    let debug_level = match cli.debug {
        0 => Level::INFO,
        1 => Level::DEBUG,
        2 => Level::TRACE,
        _ => Level::TRACE,
    };

    let builder = tracing_subscriber::fmt()
        .with_line_number(false)
        .with_file(false)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_max_level(debug_level);

    builder
        .try_init()
        .expect("Error initializing subscriber");

    let config_file = cli.config.as_deref();
    tracing::info!("Starting Forest");

    let mut config = match ForestConfig::new(config_file) {
        Ok(config) => config,
        Err(e) => {
            tracing::error!("Failed to load config: {}", e);
            return;
        }
    };

    // Print Config
    tracing::info!("Config: {}", serde_json::to_string_pretty(&config).unwrap());

    // Print Tenant Warnings
    if let Some(tenant) = &cli.tenant {
        tracing::warn!("Set Tenant: {}", tenant);
        tracing::warn!("Tenant is not implemented yet");
        config.tenant_id = None;
    } else {
        if !config.tenant_id.is_none() {
            tracing::warn!("Tenant is not implemented yet");
            config.tenant_id = None;
        }
    }

    if let Some(bind_api) = cli.bind_api {
        config.bind_api = bind_api.clone();
    }

    // create tokio runtime
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(10)
        .enable_all()
        .build()
        .unwrap();

    match &cli.command {
        Commands::Server { bind_mqtt_v3, bind_mqtt_v5 } => {
            if let Some(bind_mqtt_v3) = bind_mqtt_v3 {
                config.mqtt.bind_v3 = bind_mqtt_v3.clone();
            }
            if let Some(bind_mqtt_v5) = bind_mqtt_v5 {
                config.mqtt.bind_v5 = bind_mqtt_v5.clone();
            }
            run_server(rt, config);
        },
        Commands::Version => {
            println!("Forest Version: {}", env!("CARGO_PKG_VERSION"));
        },
        Commands::CreateBackup => {
            run_create_backup(rt, config);
        },
        Commands::CreateDevice { device_id } => {
            create_device(device_id, config);
        },
    }
}

fn run_server(rt: Runtime, config: ForestConfig) {
    setup_server_certs(&config);
    println!("Starting server");
    rt.block_on(async {
        let cancel_token = start_server(&config).await;
        tokio::select! {
            _ = cancel_token.cancelled() => {
                tracing::warn!("Server exited");
                return;
            },
            _ = tokio::signal::ctrl_c() => {},
        };
    });
}

fn run_create_backup(rt: Runtime, config: ForestConfig) {
    let api_base_url = format!("http://{}", config.bind_api);
    rt.block_on(
        async {
            let result = create_backup(&api_base_url).await;
            match result {
                Ok(msg) => {
                    tracing::info!("Backup created: {}", msg);
                },
                Err(e) => {
                    tracing::error!("Error creating backup: {}", e);
                },
            }
        }
    )
}

fn get_certificate_manager(config: &ForestConfig) -> CertificateManager {
    let tenant_id = config.tenant_id.clone();
    let cert_manager = match CertificateManager::new(&config.cert_dir, tenant_id) {
        Ok(manager) => manager,
        Err(e) => {
            tracing::error!("Failed to create certificate manager: {}", e);
            panic!("Failed to create certificate manager");
        }
    };
    cert_manager
}

fn setup_server_certs(config: &ForestConfig) {
    println!("Generating server certificates");
    let cert_manager = get_certificate_manager(config);
    let server_name = config.server_name.clone();
    let host_names: Vec<&str> = config.host_names.iter().map(|x| &**x).collect();
    match cert_manager.setup(&server_name, &host_names) {
        Ok(_) => {
            tracing::info!("Server certificates successfully set up");
        },
        Err(e) => {
            tracing::error!("Failed to set up server certificates: {}", e);
            panic!("Failed to set up server certificates");
        },
    }
}

fn create_device(device_id: &str, config: ForestConfig) {
    println!("Creating device: {}", device_id);

    let cert_manager = Arc::new(get_certificate_manager(&config));
    let db_path = PathBuf::from(&config.database.path);
    let maybe_db = DB::open_default(db_path.to_str().unwrap());
    let db = {
        match maybe_db {
            Ok(db) => Arc::new(db),
            Err(e) => {
                panic!("Failed to open RocksDB: {:?}", e);
            }
        }
    };

    let tenant_id = config.tenant_id.as_deref();
    let tenant = TenantId::from_option(tenant_id);

    match create_device_api(device_id, &tenant, db, cert_manager) {
        Ok(device) => {
            tracing::info!("Device successfully created");
            println!("\nDevice ID: \n{}", device.device_id);
            if let Some(key) = &device.key {
                println!("\nDevice Key: \n{}", key);
            }
            if let Some(cert) = &device.certificate {
                println!("\nDevice Cert: \n{}", cert);
            }
        },
        Err(e) => {
            tracing::error!("Failed to create device: {}", e);
        },
    }
}
