use std::io::PipeReader;
use std::process::{Child, Command, Stdio};
use tokio::net::windows::named_pipe::NamedPipeServer;

use crate::config::{Config, ExecType};
use crate::dotnet::load_dotnet_assembly;
use crate::firewall::allow_ports_through_firewall;

pub struct ProcessSpawner {
    pwsh_path: String,
    dotnet_path: String,
}

impl ProcessSpawner {
    pub fn new(pwsh_path: &str, dotnet_path: &str) -> Self {
        Self {
            pwsh_path: pwsh_path.to_owned(),
            dotnet_path: dotnet_path.to_owned()
        }
    }

    pub fn from_config(config: &Config) -> Self {
        Self::new(&config.general.pwsh_path, &config.general.dotnet_path)
    }

    pub fn spawn_process(
        &self,
        name: &str,
        exec_type: &ExecType,
        path: &str,
        args: &[String],
        working_dir: &str,
        ports: &[u16],
    ) -> Result<(Child, PipeReader), String> {
        // Handle firewall ports if specified
        if !ports.is_empty() {
            if let Err(err) = allow_ports_through_firewall(name, ports) {
                return Err(format!("Failed allowing {:?} in firewall, err: {err}", ports));
            }
        }

        let (reader, writer) = std::io::pipe().unwrap();

        let child = match exec_type {
            ExecType::Native => {
                Command::new(path)
                    .args(args)
                    .current_dir(working_dir)
                    .stdin(Stdio::null())
                    .stdout(writer.try_clone().unwrap())
                    .stderr(writer.try_clone().unwrap())
                    .spawn()
                    .map_err(|e| format!("Failed to start native process: {e}"))?
            },
            ExecType::Ps1 => {
                let mut cmd = Command::new(format!("{}/pwsh.exe", self.pwsh_path));
                cmd.args(["-ExecutionPolicy", "Bypass", "-File", path])
                    .args(args)
                    .current_dir(working_dir)
                    .stdin(Stdio::null())
                    .stdout(writer.try_clone().unwrap())
                    .stderr(writer.try_clone().unwrap())
                    .spawn()
                    .map_err(|e| format!("Failed to start PowerShell: {e}"))?
            }
            ExecType::Dotnet => load_dotnet_assembly(
                &self.dotnet_path,
                path,
                Some(&args.join(" ")),
                working_dir,
                writer,
            )
            .map_err(|e| format!("Failed to start dotnet: {e}"))?,
            ExecType::Cmd => {
                let mut cmd = Command::new("cmd.exe");
                cmd.arg("/C")
                    .arg(path)
                    .args(args)
                    .current_dir(working_dir)
                    .stdin(Stdio::null())
                    .stdout(writer.try_clone().unwrap())
                    .stderr(writer.try_clone().unwrap())
                    .spawn()
                    .map_err(|e| format!("Failed to start cmd.exe: {e}"))?
            }
            ExecType::PELoad => {
                crate::pe::solstice_reflective_load_pe(path, args, working_dir, writer)
                    .map_err(|e| format!("Failed to load PE via reflective loading {e}"))?
            }
            ExecType::Msbuild => {
                let dotnet_exe = if self.dotnet_path.ends_with("dotnet.exe") {
                    self.dotnet_path.clone()
                } else {
                    format!(
                        "{}/dotnet.exe",
                        self.dotnet_path.trim_end_matches('/')
                    )
                };
                let mut cmd = Command::new(dotnet_exe);
                cmd.arg("msbuild")
                    .arg(path)
                    .args(args)
                    .current_dir(working_dir)
                    .stdin(Stdio::null())
                    .stdout(writer.try_clone().unwrap())
                    .stderr(writer.try_clone().unwrap())
                    .spawn()
                    .map_err(|e| format!("Failed to start dotnet msbuild: {e}"))?
            }
        };

        Ok((child, reader))
    }
} 