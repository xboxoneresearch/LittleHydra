use log::{error, info};
use std::io::{BufRead, BufReader, PipeReader};
use std::thread;

pub struct ProcessLogWriter {
    service_name: String,
}

impl ProcessLogWriter {
    pub fn new(service_name: String) -> Self {
        Self { service_name }
    }

    pub fn log_line(&self, line: &str) {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            let log_message = format!("[{}] {}", self.service_name, trimmed);
            info!("{log_message}");
        }
    }
}

pub fn spawn_output_logger(
    service_name: String,
    output_reader: PipeReader,
) {
    let service_name_clone = service_name.clone();
    thread::spawn(move || {
        let writer = ProcessLogWriter::new(service_name_clone.clone());
        let reader = BufReader::new(output_reader);
        for line in reader.lines() {
            match line {
                Ok(line) => writer.log_line(&line),
                Err(e) => error!("[{service_name_clone}] Failed to read output: {e}"),
            }
        }
    });
}
