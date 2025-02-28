extern crate forest;

use forest::config::ForestConfig;
use forest::server::start_server;
use forest::cli::{Cli, Commands};
use forest::api::client::create_backup;
use forest::certs::CertificateManager;
use tokio::runtime::Runtime;
use tracing::Level;
use clap::Parser;

fn main() {
    let cli = Cli::parse();

    let debug_level = match cli.debug {
        0 => Level::WARN,
        1 => Level::INFO,
        2 => Level::DEBUG,
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

    // Print Tenant
    if let Some(tenant) = &cli.tenant {
        tracing::warn!("Set Tenant: {}", tenant);
        config.tenant_id = tenant.clone();
    } else {
        tracing::info!("Using default tenant");
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
    let cert_manager = match CertificateManager::new(&config.cert_dir, Some(tenant_id)) {
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
    let cert_manager = get_certificate_manager(&config);
    match cert_manager.create_client_cert(device_id) {
        Ok(_) => {
            tracing::info!("Device certificate successfully created");
        },
        Err(e) => {
            tracing::error!("Failed to create device certificate: {}", e);
            panic!("Failed to create device certificate");
        },
    }
    // println!("\nDevice Cert: \n{}", device_cert.cert);
    // println!("\nDevice Key: \n{}", device_cert.key);
}
