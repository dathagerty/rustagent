use anyhow::Result;
use std::path::PathBuf;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub fn init_logging() -> Result<WorkerGuard> {
    let log_dir = get_log_directory()?;
    std::fs::create_dir_all(&log_dir)?;

    let file_appender = RollingFileAppender::new(Rotation::DAILY, &log_dir, "rustagent.log");

    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let file_layer = fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_target(true)
        .with_thread_ids(true);

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("rustagent=info"));

    tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .init();

    Ok(guard)
}

fn get_log_directory() -> Result<PathBuf> {
    if let Ok(state_home) = std::env::var("XDG_STATE_HOME") {
        return Ok(PathBuf::from(state_home).join("rustagent").join("logs"));
    }

    if let Some(data_dir) = dirs::data_local_dir() {
        return Ok(data_dir.join("rustagent").join("logs"));
    }

    if let Some(home) = dirs::home_dir() {
        return Ok(home
            .join(".local")
            .join("state")
            .join("rustagent")
            .join("logs"));
    }

    anyhow::bail!("Could not determine log directory")
}

pub fn get_log_directory_path() -> Option<PathBuf> {
    get_log_directory().ok()
}
