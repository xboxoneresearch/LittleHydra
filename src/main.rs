mod assets;
mod cli;
mod config;
mod dotnet;
mod error;
mod firewall;
mod pe;
mod process_log_writer;
mod process_manager;
mod rpc;
mod tcp_log_writer;

use clap::Parser;
use flexi_logger::FileSpec;
use flexi_logger::{Logger, WriteMode};
use log::*;
use std::fs;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::signal;

use crate::cli::Cli;
use crate::config::Config;
use crate::error::Error;
use crate::firewall::{allow_port_through_firewall, disable_firewalls};
use crate::process_manager::ProcessManager;
use crate::rpc::named_pipe_ipc_server;
use crate::tcp_log_writer::TcpLogWriter;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let cli = Cli::parse();

    let config_str = fs::read_to_string("config.toml")?;
    let config: Config = toml::from_str(&config_str)?;
    println!("General config: {:#?}", config.general);
    println!("Loaded services: {:#?}", config.service);

    let config = Arc::new(config);
    let pm = Arc::new(ProcessManager::new(config.clone()));

    // Set up flexi_logger with file and stdout initially
    let log_level = cli.get_log_level();
    let mut logger = Logger::try_with_str(log_level.to_string())?
        .log_to_file(FileSpec::default())
        .duplicate_to_stderr(flexi_logger::Duplicate::All)
        .write_mode(WriteMode::BufferAndFlush);

    // Connect to log_host if provided via command line
    let tcp_writer = tcp_log_writer::init_tcp_writer(cli.log_host.as_ref());
    // Add tcp logger
    if let Some(ref tcp_stream) = tcp_writer {
        let tcp_writer = TcpLogWriter {
            stream: tcp_stream.clone(),
        };
        logger = logger.add_writer("tcp", Box::new(tcp_writer));
    }

    logger.start()?;
    info!("LittleHydra starting up...");

    // Print current working directory
    match std::env::current_dir() {
        Ok(path) => info!("Current working directory: {}", path.display()),
        Err(e) => error!("Failed to get current working directory: {e}"),
    }

    // Start the named pipe server in a background thread
    let pm_clone = pm.clone();
    let _ = std::thread::spawn(move || {
        let rt = Runtime::new().unwrap();
        rt.block_on(named_pipe_ipc_server(pm_clone));
    });

    // Start the TCP RPC server in a background thread
    #[cfg(feature = "network_server")]
    {
        let pm_clone = pm.clone();
        let rpc_port = config.general.rpc_port;
        disable_firewalls()?;
        allow_port_through_firewall("LittleHydra", rpc_port)?;
        std::thread::spawn(move || {
            let rt = Runtime::new().unwrap();
            rt.block_on(crate::rpc::tcp_rpc_server(rpc_port, pm_clone));
        });
    }

    // Sort by start_priority
    let mut services = config.service.clone();
    services.sort_by_key(|s| s.start_priority);

    for svc in services {
        // Informational output only; actual process management is handled by ProcessManager
        let name = svc.name.clone();
        let path = svc.path.clone();
        let args = svc.args.clone();
        let exec_type = svc.exec_type;
        let working_dir = svc.working_dir.clone();
        info!("[{exec_type:?}] Configured: {path} {args:?} in {working_dir}");

        match pm.start_service(&name) {
            Ok(()) => {
                info!("Service {name} started successfully")
            }
            Err(err) => {
                error!("Failed to launch service {name}, error: {err}")
            }
        };
    }

    info!("Looping until exit is requested...");
    signal::ctrl_c().await?;
    info!("Exiting...");
    Ok(())
}
