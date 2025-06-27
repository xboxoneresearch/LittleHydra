use chrono::Utc;
use std::collections::HashMap;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

use crate::config::{Config, ExecType};
use crate::dotnet::load_dotnet_assembly_with_config;
use crate::process_log_writer::spawn_output_logger;
use crate::rpc::{ServiceState, ServiceStatusState};

pub struct ProcessManager {
    pub handles: Arc<Mutex<HashMap<String, Child>>>,
    pub states: Arc<Mutex<HashMap<String, ServiceState>>>,
    pub config: Arc<Config>,
}

impl ProcessManager {
    pub fn new(config: Arc<Config>) -> Self {
        let mut state_map = HashMap::new();
        for svc in &config.service {
            state_map.insert(
                svc.name.clone(),
                ServiceState {
                    state: ServiceStatusState::Stopped,
                    exit_code: Some(0),
                    start_time: None,
                    stop_time: None,
                },
            );
        }
        Self {
            handles: Arc::new(Mutex::new(HashMap::new())),
            states: Arc::new(Mutex::new(state_map)),
            config,
        }
    }

    pub fn get_states(&self) -> HashMap<String, ServiceState> {
        self.states.lock().unwrap().clone()
    }

    pub fn start_service(&self, name: &str) -> Result<(), String> {
        let svc = self
            .config
            .service
            .iter()
            .find(|s| s.name == name)
            .ok_or_else(|| format!("Service '{name}' not found"))?;
        let mut handles = self.handles.lock().unwrap();
        if handles.contains_key(name) {
            return Err(format!("Service '{name}' is already running"));
        }
        let mut child = match svc.exec_type {
            ExecType::Ps1 => {
                let mut cmd = Command::new(format!("{}/pwsh.exe", self.config.general.pwsh_path));
                cmd.args(["-ExecutionPolicy", "Bypass", "-File", &svc.path])
                    .args(&svc.args)
                    .current_dir(&svc.working_dir)
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .map_err(|e| format!("Failed to start PowerShell: {e}"))?
            }
            ExecType::Dotnet => load_dotnet_assembly_with_config(
                &self.config,
                &svc.path,
                Some(&svc.args.join("")),
                &svc.working_dir,
            )
            .map_err(|e| format!("Failed to start dotnet: {e}"))?,
            ExecType::Cmd => {
                let mut cmd = Command::new("cmd.exe");
                cmd.arg("/C")
                    .arg(&svc.path)
                    .args(&svc.args)
                    .current_dir(&svc.working_dir)
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .map_err(|e| format!("Failed to start cmd.exe: {e}"))?
            }
            ExecType::Native => {
                // Placeholder for solstice_loader integration
                crate::pe::solstice_reflective_load_pe(&svc.path, &svc.args, &svc.working_dir)
                    .map_err(|e| format!("Failed to load PE via reflective loading {e}"))?
            }
            ExecType::Msbuild => {
                // Use dotnet msbuild to build the project at svc.path with args
                let dotnet_exe = if self.config.general.dotnet_path.ends_with("dotnet.exe") {
                    self.config.general.dotnet_path.clone()
                } else {
                    format!(
                        "{}/dotnet.exe",
                        self.config.general.dotnet_path.trim_end_matches('/')
                    )
                };
                let mut cmd = Command::new(dotnet_exe);
                cmd.arg("msbuild")
                    .arg(&svc.path)
                    .args(&svc.args)
                    .current_dir(&svc.working_dir)
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());
                cmd.spawn()
                    .map_err(|e| format!("Failed to start dotnet msbuild: {e}"))?
            }
        };

        // Capture stdout and stderr for logging
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        // Spawn output logger threads
        spawn_output_logger(name.to_string(), stdout, stderr);

        handles.insert(name.to_string(), child);
        self.states.lock().unwrap().insert(
            name.to_string(),
            ServiceState {
                state: ServiceStatusState::Running,
                exit_code: None,
                start_time: Some(Utc::now()),
                stop_time: None,
            },
        );
        Ok(())
    }

    pub fn stop_service(&self, name: &str) -> Result<(), String> {
        let mut handles = self.handles.lock().unwrap();
        if let Some(mut child) = handles.remove(name) {
            let _ = child.kill();
            let exit_status = child.wait().ok();
            let exit_code = exit_status.and_then(|s| s.code());
            self.states.lock().unwrap().insert(
                name.to_string(),
                ServiceState {
                    state: ServiceStatusState::Stopped,
                    exit_code,
                    start_time: self
                        .states
                        .lock()
                        .unwrap()
                        .get(name)
                        .and_then(|s| s.start_time),
                    stop_time: Some(Utc::now()),
                },
            );
            Ok(())
        } else {
            Err(format!("Service '{name}' is not running"))
        }
    }

    pub fn add_service(&self, name: &str, config: serde_json::Value) -> Result<(), String> {
        use crate::config::ServiceConfig;
        let mut svc: ServiceConfig = serde_json::from_value(config)
            .map_err(|e| format!("Failed to parse service config: {e}"))?;
        svc.name = name.to_string();
        // Add to config
        let config_clone = &mut self.config.clone();
        let config = Arc::get_mut(config_clone).ok_or("Failed to get mutable config")?;
        if config.service.iter().any(|s| s.name == name) {
            return Err(format!("Service '{name}' already exists"));
        }
        config.service.push(svc.clone());
        // Add to states
        self.states.lock().unwrap().insert(
            name.to_string(),
            ServiceState {
                state: ServiceStatusState::Stopped,
                exit_code: Some(0),
                start_time: None,
                stop_time: None,
            },
        );
        Ok(())
    }

    pub fn delete_service(&self, name: &str) -> Result<(), String> {
        let config_clone = &mut self.config.clone();
        let config = Arc::get_mut(config_clone).ok_or("Failed to get mutable config")?;
        let orig_len = self.config.service.len();
        config.service.retain(|s| s.name != name);
        if config.service.len() == orig_len {
            return Err(format!("Service '{name}' not found"));
        }
        self.states.lock().unwrap().remove(name);
        Ok(())
    }

    pub fn save_config(&self) -> Result<(), String> {
        let config = &*self.config;
        let toml =
            toml::to_string(config).map_err(|e| format!("Failed to serialize config: {e}"))?;
        std::fs::write("config.toml", toml)
            .map_err(|e| format!("Failed to write config.toml: {e}"))?;
        Ok(())
    }
}
