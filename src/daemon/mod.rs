pub mod api;
pub mod client;
pub mod server;
pub mod static_files;
pub mod ws;

use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct DaemonConfig {
    pub bind_address: String,
    pub port: u16,
    pub pid_file: PathBuf,
    pub log_dir: PathBuf,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("rustagent");
        let state_dir = dirs::state_dir()
            .or_else(dirs::data_dir)
            .unwrap_or_else(|| PathBuf::from("."))
            .join("rustagent")
            .join("logs");

        Self {
            bind_address: "127.0.0.1".to_string(),
            port: 7400,
            pid_file: data_dir.join("rustagent.pid"),
            log_dir: state_dir,
        }
    }
}

impl DaemonConfig {
    pub fn socket_addr(&self) -> Result<SocketAddr, std::net::AddrParseError> {
        format!("{}:{}", self.bind_address, self.port).parse()
    }
}

/// Write the current process PID to the PID file
pub fn write_pid_file(config: &DaemonConfig) -> anyhow::Result<()> {
    if let Some(parent) = config.pid_file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&config.pid_file, std::process::id().to_string())?;
    Ok(())
}

/// Read the PID from the PID file, or None if it doesn't exist
pub fn read_pid_file(config: &DaemonConfig) -> anyhow::Result<Option<u32>> {
    match std::fs::read_to_string(&config.pid_file) {
        Ok(content) => {
            let pid: u32 = content.trim().parse()?;
            Ok(Some(pid))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Remove the PID file
pub fn remove_pid_file(config: &DaemonConfig) -> anyhow::Result<()> {
    match std::fs::remove_file(&config.pid_file) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// Check if the daemon is running by reading PID file and checking process liveness
pub fn is_daemon_running(config: &DaemonConfig) -> anyhow::Result<bool> {
    match read_pid_file(config)? {
        Some(pid) => {
            #[cfg(unix)]
            {
                let result = unsafe { libc::kill(pid as i32, 0) };
                Ok(result == 0)
            }
            #[cfg(not(unix))]
            {
                let _ = pid;
                Ok(true)
            }
        }
        None => Ok(false),
    }
}
