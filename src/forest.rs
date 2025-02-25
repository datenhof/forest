extern crate forest;

use std::path::Path;

use forest::config::ForestConfig;
use forest::server::start_server;
use forest::cli::{Cli, Commands};
use forest::api::client::create_backup;
use forest::certs::generate_client_certificate;
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

    let mut config = ForestConfig::new(config_file).unwrap();

    // Print Config
    tracing::info!("Config: {}", serde_json::to_string_pretty(&config).unwrap());

    // Print Tenant
    if let Some(tenant) = &cli.tenant {
        tracing::warn!("Default Tenant: {}", tenant);
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

fn create_device(device_id: &str, config: ForestConfig) {
    tracing::warn!("Creating device: {}", device_id);
    // get cfssl directory
    let cfssl_path= config.cfssl_path.map(|dir| Path::new(&dir).to_path_buf());
    let cert_dir = config.cert_dir.map(|dir| Path::new(&dir).to_path_buf());
    // make sure cert_dir is set
    if cert_dir.is_none() {
        tracing::error!("ssl_cert_dir is not set in config");
        return;
    }
    let device_cert = generate_client_certificate(device_id, &cert_dir.unwrap(), cfssl_path.as_deref(), None).unwrap();
    println!("\nDevice Cert: \n{}", device_cert.cert);
    println!("\nDevice Key: \n{}", device_cert.key);
}
