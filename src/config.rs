use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum ExecType {
    // Executes a script via `cmd.exe /c`
    Cmd,
    // Executes a script via `pwsh.exe -ExecutionPolicy Bypass -File`
    Ps1,
    // Executes a dotnet assembly via: `Assembly.Load(..); Assembly.Entry; Assembly.Invoke(null, args)`
    Dotnet,
    // Executes a dotnet msbuild task via `dotnet msbuild`
    Msbuild,
    // Starts a new suspended Win32 process and injects PE loader shellcode into it
    Native,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServiceConfig {
    pub name: String,
    pub exec_type: ExecType,
    pub path: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub working_dir: String,
    pub start_priority: u32,
    pub restart_on_error: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GeneralConfig {
    pub dotnet_path: String,
    pub pwsh_path: String,
    pub rpc_port: u16,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub general: GeneralConfig,
    pub service: Vec<ServiceConfig>,
}
