
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Logger initialization failed: {0}")]
    LoggerInit(#[from] flexi_logger::FlexiLoggerError),
    #[error("Failed to read config file: {0}")]
    ConfigRead(#[from] std::io::Error),
    #[error("Failed to parse config: {0}")]
    ConfigParse(#[from] toml::de::Error),
    #[error("Windows API error: {0}")]
    Windows(#[from] windows::core::Error),
    #[error("Process creation failed: {0}")]
    ProcessCreation(String),
    #[error("Memory allocation failed: {0}")]
    MemoryAllocation(String),
    #[error("Process memory write failed: {0}")]
    ProcessMemoryWrite(String),
    #[error("Thread creation failed: {0}")]
    ThreadCreation(String),
    #[error("Firewall operation failed: {0}")]
    Firewall(String),
    #[error("COM initialization failed: {0}")]
    ComInit(String),
    #[error("Impersonation error: {0}")]
    Impersonation(String),
}
