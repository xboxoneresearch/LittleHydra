use flexi_logger::writers::LogWriter;
use std::io::{self, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};

pub struct TcpLogWriter {
    pub stream: Arc<Mutex<TcpStream>>,
}

impl LogWriter for TcpLogWriter {
    fn write(&self, now: &mut flexi_logger::DeferredNow, record: &log::Record) -> io::Result<()> {
        let mut stream = self.stream.lock().unwrap();
        writeln!(
            stream,
            "{} [{}] {}",
            now.now().format("%Y-%m-%d %H:%M:%S"),
            record.level(),
            &record.args()
        )
    }
    fn flush(&self) -> io::Result<()> {
        let mut stream = self.stream.lock().unwrap();
        stream.flush()
    }
}

pub fn init_tcp_writer(log_host: Option<&String>) -> Option<Arc<Mutex<TcpStream>>> {
    if let Some(log_host) = log_host {
        match log_host.split_once(":") {
            Some((host, port_str)) => match port_str.parse::<u16>() {
                Ok(port) => match TcpStream::connect((host, port)) {
                    Ok(stream) => {
                        println!("Connected to log host at {host}:{port}");
                        Some(Arc::new(Mutex::new(stream)))
                    }
                    Err(e) => {
                        println!("Failed to connect to log host {host}:{port}: {e}");
                        None
                    }
                },
                Err(e) => {
                    println!("Invalid port in log_host '{port_str}': {e}");
                    None
                }
            },
            None => {
                println!(
                    "Invalid log_host format: '{log_host}'. Expected <host>:<port>"
                );
                None
            }
        }
    } else {
        None
    }
}
