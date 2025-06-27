use log::{error, info};
use std::io::{BufRead, BufReader};
use std::process::{ChildStderr, ChildStdout};
use std::thread;

pub struct ProcessLogWriter {
    service_name: String,
}

impl ProcessLogWriter {
    pub fn new(service_name: String) -> Self {
        Self { service_name }
    }

    pub fn log_line(&self, line: &str, is_stderr: bool) {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            let level = if is_stderr { "ERROR" } else { "INFO" };
            let log_message = format!("[{}] {}: {}", self.service_name, level, trimmed);

            if is_stderr {
                error!("{log_message}");
            } else {
                info!("{log_message}");
            }
        }
    }
}

pub fn spawn_output_logger(
    service_name: String,
    stdout: Option<ChildStdout>,
    stderr: Option<ChildStderr>,
) {
    // Spawn thread for stdout
    if let Some(stdout) = stdout {
        let service_name_clone = service_name.clone();
        thread::spawn(move || {
            let writer = ProcessLogWriter::new(service_name_clone.clone());
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                match line {
                    Ok(line) => writer.log_line(&line, false),
                    Err(e) => error!("[{service_name_clone}] Failed to read stdout: {e}"),
                }
            }
        });
    }

    // Spawn thread for stderr
    if let Some(stderr) = stderr {
        let service_name_clone = service_name.clone();
        thread::spawn(move || {
            let writer = ProcessLogWriter::new(service_name_clone.clone());
            let reader = BufReader::new(stderr);
            for line in reader.lines() {
                match line {
                    Ok(line) => writer.log_line(&line, true),
                    Err(e) => error!("[{service_name_clone}] Failed to read stderr: {e}"),
                }
            }
        });
    }
}
