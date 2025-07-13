use chrono::Utc;
use ::serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::{Arc, Mutex};
use std::io::{BufReader, Read};
use base64::prelude::*;

use crate::config::{Config, ExecType};
use crate::process_log_writer::spawn_output_logger;
use crate::process_spawner::ProcessSpawner;
use crate::rpc::{ServiceState, ServiceStatusState};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OneshotConfig {
    pub exec_type: ExecType,
    pub path: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub working_dir: String,
    #[serde(default)]
    pub ports: Vec<u16>,
}

pub struct OneshotProcess {
    pub child: Child,
    // TODO: Maybe replace this with a common pipe for stderr/stdout to write into?
    pub stdout_buffer: Arc<Mutex<Vec<u8>>>,
    pub stderr_buffer: Arc<Mutex<Vec<u8>>>,
    pub exit_status: Option<i32>,
}

pub struct ProcessManager {
    pub config_path: PathBuf,
    pub handles: Arc<Mutex<HashMap<String, Child>>>,
    pub states: Arc<Mutex<HashMap<String, ServiceState>>>,
    pub config: Arc<Config>,
    pub spawner: ProcessSpawner,
    pub oneshot_processes: Arc<Mutex<HashMap<u32, OneshotProcess>>>,
}

impl ProcessManager {
    pub fn new(config: Arc<Config>, config_path: &Path) -> Self {
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
            config_path: config_path.to_path_buf(),
            handles: Arc::new(Mutex::new(HashMap::new())),
            states: Arc::new(Mutex::new(state_map)),
            config: config.clone(),
            spawner: ProcessSpawner::from_config(&config),
            oneshot_processes: Arc::new(Mutex::new(HashMap::new())),
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

        let mut child = self.spawner.spawn_process(
            name,
            &svc.exec_type,
            &svc.path,
            &svc.args,
            &svc.working_dir,
            &svc.ports,
        )?;

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
        std::fs::write(&self.config_path, toml)
            .map_err(|e| format!("Failed to write config.toml: {e}"))?;
        Ok(())
    }

    pub fn oneshot_spawn(&self, name: &str, config: serde_json::Value) -> Result<u32, String> {
        let oneshot_config: OneshotConfig = serde_json::from_value(config)
            .map_err(|e| format!("Failed to parse oneshot config: {e}"))?;

        let mut child = self.spawner.spawn_process(
            name,
            &oneshot_config.exec_type,
            &oneshot_config.path,
            &oneshot_config.args,
            &oneshot_config.working_dir,
            &oneshot_config.ports,
        )?;

        let pid = child.id();
        
        // Capture stdout and stderr for buffering
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let stdout_buffer = Arc::new(Mutex::new(Vec::new()));
        let stderr_buffer = Arc::new(Mutex::new(Vec::new()));

        let oneshot_process = OneshotProcess {
            child,
            stdout_buffer: stdout_buffer.clone(),
            stderr_buffer: stderr_buffer.clone(),
            exit_status: None,
        };

        // Spawn threads to read stdout and stderr into buffers
        if let Some(stdout) = stdout {
            let stdout_buffer_clone = Arc::clone(&stdout_buffer);
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stdout);
                let mut buf = [0; 1024];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => break, // EOF
                        Ok(n) => {
                            let mut buffer = stdout_buffer_clone.lock().unwrap();
                            buffer.extend_from_slice(&buf[..n]);
                        }
                        Err(_) => break,
                    }
                }
            });
        }

        if let Some(stderr) = stderr {
            let stderr_buffer_clone = Arc::clone(&stderr_buffer);
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stderr);
                let mut buf = [0; 1024];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => break, // EOF
                        Ok(n) => {
                            let mut buffer = stderr_buffer_clone.lock().unwrap();
                            buffer.extend_from_slice(&buf[..n]);
                        }
                        Err(_) => break,
                    }
                }
            });
        }

        // Store the Arc<Mutex<Vec<u8>>> references in the oneshot process
        // We'll need to modify the OneshotProcess struct to handle this properly
        // For now, let's use a simpler approach with separate tracking
        self.oneshot_processes.lock().unwrap().insert(pid, oneshot_process);
        Ok(pid)
    }

    pub fn oneshot_status(&self, pid: u32) -> Result<(String, String, Option<i32>), String> {
        let mut processes = self.oneshot_processes.lock().unwrap();
        
        if let Some(oneshot_process) = processes.get_mut(&pid) {
            // Check if process has exited
            if oneshot_process.exit_status.is_none() {
                match oneshot_process.child.try_wait() {
                    Ok(Some(status)) => {
                        oneshot_process.exit_status = status.code();
                    }
                    Ok(None) => {
                        // Process is still running
                    }
                    Err(_) => {
                        // Process has exited with error
                        oneshot_process.exit_status = None;
                    }
                }
            }

            // Convert buffers to base64
            let stdout_b64 = BASE64_STANDARD.encode(&*oneshot_process.stdout_buffer.lock().unwrap());
            let stderr_b64 = BASE64_STANDARD.encode(&*oneshot_process.stderr_buffer.lock().unwrap());
            let exit_status = oneshot_process.exit_status;

            // Remove the process from tracking if it has exited
            if exit_status.is_some() {
                processes.remove(&pid);
            }

            Ok((stdout_b64, stderr_b64, exit_status))
        } else {
            Err(format!("Oneshot process with PID {} not found", pid))
        }
    }

    /// Starts a background thread that monitors all running services and updates their state if they exit.
    pub fn start_monitoring(&self) {
        let handles = Arc::clone(&self.handles);
        let states = Arc::clone(&self.states);
        let config = Arc::clone(&self.config);
        let pm_self = self.clone_for_monitoring();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
                let mut to_remove = Vec::new();
                let mut to_restart = Vec::new();
                {
                    let mut handles_guard = handles.lock().unwrap();
                    for (name, child) in handles_guard.iter_mut() {
                        match child.try_wait() {
                            Ok(Some(status)) => {
                                let exit_code = status.code();
                                let mut states_guard = states.lock().unwrap();
                                if let Some(state) = states_guard.get_mut(name) {
                                    state.state = ServiceStatusState::Stopped;
                                    state.exit_code = exit_code;
                                    state.stop_time = Some(Utc::now());
                                }
                                // Find service in ServiceConfig by name and check restart_on_error
                                if let Some(svc) = config.service.iter().find(|s| s.name == *name) {
                                    if svc.restart_on_error && exit_code.unwrap_or(0) != 0 {
                                        to_restart.push(name.clone());
                                    }
                                }
                                to_remove.push(name.clone());
                            }
                            Ok(None) => {}
                            Err(_e) => {
                                let mut states_guard = states.lock().unwrap();
                                if let Some(state) = states_guard.get_mut(name) {
                                    state.state = ServiceStatusState::Stopped;
                                    state.exit_code = None;
                                    state.stop_time = Some(Utc::now());
                                }
                                to_remove.push(name.clone());
                            }
                        }
                    }
                    for name in to_remove {
                        handles_guard.remove(&name);
                    }
                }
                // Restart services outside the lock
                for name in to_restart {
                    let _ = pm_self.start_service(&name);
                }
            }
        });
    }

    // Helper to allow calling start_service from monitoring thread
    fn clone_for_monitoring(&self) -> Arc<ProcessManager> {
        Arc::new(ProcessManager {
            config_path: self.config_path.clone(),
            handles: Arc::clone(&self.handles),
            states: Arc::clone(&self.states),
            config: Arc::clone(&self.config),
            spawner: ProcessSpawner::from_config(&self.config),
            oneshot_processes: Arc::clone(&self.oneshot_processes),
        })
    }
}
