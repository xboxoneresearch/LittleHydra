use crate::process_manager::ProcessManager;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};

const CURRENT_PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ServiceStatusState {
    Running,
    Stopped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceState {
    pub state: ServiceStatusState,
    pub exit_code: Option<i32>,
    pub start_time: Option<DateTime<Utc>>,
    pub stop_time: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase")]
pub enum RpcRequest {
    Info,
    ListServices,
    StartService {
        name: String,
    },
    StopService {
        name: String,
    },
    AddService {
        name: String,
        config: serde_json::Value,
    },
    DeleteService {
        name: String,
    },
    GetConfig,
    SaveConfig,
    OpenFirewallPorts {
        name: String,
        ports: Vec<u16>,
    },
    DeleteFirewallRule {
        name: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum RpcResponse {
    Success { data: serde_json::Value },
    Error { message: String },
}

pub async fn handle_rpc_stream<T>(stream: T, pm: Arc<ProcessManager>)
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => break, // EOF
            Ok(_) => {
                let req: Result<RpcRequest, _> = serde_json::from_str(&line);
                let resp = match req {
                    Ok(RpcRequest::Info) => {
                        RpcResponse::Success {
                            data: serde_json::json!({
                                "app_version": env!("CARGO_PKG_VERSION"),
                                "build_date": env!("BUILD_DATE"),
                                "protocol_version": CURRENT_PROTOCOL_VERSION
                            }),
                        }
                    },
                    Ok(RpcRequest::ListServices) => {
                        let states = pm.get_states();
                        RpcResponse::Success {
                            data: serde_json::to_value(states).unwrap(),
                        }
                    },
                    Ok(RpcRequest::StartService { name }) => match pm.start_service(&name) {
                        Ok(()) => RpcResponse::Success {
                            data: serde_json::json!({"name": name, "state": "Running"}),
                        },
                        Err(e) => RpcResponse::Error { message: e },
                    },
                    Ok(RpcRequest::StopService { name }) => match pm.stop_service(&name) {
                        Ok(exit_code) => RpcResponse::Success {
                            data: serde_json::json!({"name": name, "state": "Stopped", "exit_code": exit_code}),
                        },
                        Err(e) => RpcResponse::Error { message: e },
                    },
                    Ok(RpcRequest::AddService { name, config }) => {
                        match pm.add_service(&name, config) {
                            Ok(()) => RpcResponse::Success {
                                data: serde_json::json!({"name": name, "status": "Added"}),
                            },
                            Err(e) => RpcResponse::Error { message: e },
                        }
                    }
                    Ok(RpcRequest::DeleteService { name }) => match pm.delete_service(&name) {
                        Ok(()) => RpcResponse::Success {
                            data: serde_json::json!({"name": name, "status": "Deleted"}),
                        },
                        Err(e) => RpcResponse::Error { message: e },
                    },
                    Ok(RpcRequest::GetConfig) => {
                        let config = (*pm.config).clone();
                        RpcResponse::Success {
                            data: serde_json::to_value(config).unwrap(),
                        }
                    },
                    Ok(RpcRequest::SaveConfig) => match pm.save_config() {
                        Ok(()) => RpcResponse::Success {
                            data: serde_json::json!({"status": "ConfigSaved"}),
                        },
                        Err(e) => RpcResponse::Error { message: e },
                    },
                    Ok(RpcRequest::OpenFirewallPorts { name, ports }) => {
                        match crate::firewall::allow_ports_through_firewall(&name, &ports) {
                            Ok(()) => RpcResponse::Success {
                                data: serde_json::json!({"name": name, "ports": ports, "status": "PortsOpened"}),
                            },
                            Err(e) => RpcResponse::Error { message: format!("Failed to open firewall ports: {e}") },
                        }
                    }
                    Ok(RpcRequest::DeleteFirewallRule { name }) => {
                        match crate::firewall::remove_port_from_firewall_by_name(&name) {
                            Ok(()) => RpcResponse::Success {
                                data: serde_json::json!({"name": name, "status": "FirewallRuleDeleted"}),
                            },
                            Err(e) => RpcResponse::Error { message: format!("Failed to delete firewall rule: {e}") },
                        }
                    }
                    Err(e) => RpcResponse::Error {
                        message: format!("Invalid request: {e}"),
                    },
                };
                let resp_str = serde_json::to_string(&resp).unwrap() + "\n";
                let _ = reader.get_mut().write_all(resp_str.as_bytes()).await;
            }
            Err(e) => {
                println!("[RPC] Read error: {e}");
                break;
            }
        }
    }
}

pub async fn named_pipe_ipc_server(pm: Arc<ProcessManager>) {
    use tokio::net::windows::named_pipe::ServerOptions;
    let pipe_name = r"\\.\pipe\little_hydra_rpc";
    println!("[RPC] Named pipe server listening on {pipe_name}");

    loop {
        let server = ServerOptions::new()
            .first_pipe_instance(true)
            .max_instances(1)
            .create(pipe_name)
            .expect("Failed to create named pipe server");

        println!("[RPC] Waiting for client connection...");
        server.connect().await.expect("Failed to connect client");
        println!("[RPC] Client connected!");

        handle_rpc_stream(server, pm.clone()).await;
    }
}

#[cfg(feature = "network_server")]
pub async fn tcp_rpc_server(port: u16, pm: Arc<ProcessManager>) {
    use tokio::net::TcpListener;
    use tokio::task;
    let addr = format!("0.0.0.0:{port}");
    let listener = TcpListener::bind(&addr)
        .await
        .expect("Failed to bind TCP RPC port");
    println!("[RPC] TCP server listening on {addr}");
    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                println!("[RPC] TCP client connected: {addr}");
                let pm = pm.clone();
                task::spawn(async move {
                    handle_rpc_stream(stream, pm).await;
                });
            }
            Err(e) => {
                println!("[RPC] TCP accept error: {e}");
            }
        }
    }
}
