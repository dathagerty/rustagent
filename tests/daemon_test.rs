use rustagent::daemon::*;
use tempfile::TempDir;

#[test]
fn test_daemon_config_defaults() {
    let config = DaemonConfig::default();
    assert_eq!(config.bind_address, "127.0.0.1");
    assert_eq!(config.port, 7400);
    assert!(config.pid_file.to_string_lossy().contains("rustagent.pid"));
}

#[test]
fn test_socket_addr() {
    let config = DaemonConfig::default();
    let addr = config.socket_addr().unwrap();
    assert_eq!(addr.port(), 7400);
    assert_eq!(addr.ip().to_string(), "127.0.0.1");
}

#[test]
fn test_pid_file_write_read() {
    let tmp = TempDir::new().unwrap();
    let config = DaemonConfig {
        pid_file: tmp.path().join("test.pid"),
        ..DaemonConfig::default()
    };

    write_pid_file(&config).unwrap();
    let pid = read_pid_file(&config).unwrap();
    assert_eq!(pid, Some(std::process::id()));
}

#[test]
fn test_pid_file_read_nonexistent() {
    let tmp = TempDir::new().unwrap();
    let config = DaemonConfig {
        pid_file: tmp.path().join("nonexistent.pid"),
        ..DaemonConfig::default()
    };

    assert_eq!(read_pid_file(&config).unwrap(), None);
}

#[test]
fn test_pid_file_remove() {
    let tmp = TempDir::new().unwrap();
    let config = DaemonConfig {
        pid_file: tmp.path().join("test.pid"),
        ..DaemonConfig::default()
    };

    write_pid_file(&config).unwrap();
    remove_pid_file(&config).unwrap();
    assert_eq!(read_pid_file(&config).unwrap(), None);
}

#[test]
fn test_remove_nonexistent_pid_file() {
    let tmp = TempDir::new().unwrap();
    let config = DaemonConfig {
        pid_file: tmp.path().join("nonexistent.pid"),
        ..DaemonConfig::default()
    };
    // Should not error
    remove_pid_file(&config).unwrap();
}

#[test]
fn test_is_daemon_running_with_live_pid() {
    let tmp = TempDir::new().unwrap();
    let config = DaemonConfig {
        pid_file: tmp.path().join("test.pid"),
        ..DaemonConfig::default()
    };
    write_pid_file(&config).unwrap();
    assert!(is_daemon_running(&config).unwrap());
}

#[test]
fn test_is_daemon_running_with_dead_pid() {
    let tmp = TempDir::new().unwrap();
    let config = DaemonConfig {
        pid_file: tmp.path().join("test.pid"),
        ..DaemonConfig::default()
    };
    // Write a PID that almost certainly doesn't exist
    std::fs::write(&config.pid_file, "99999999").unwrap();
    assert!(!is_daemon_running(&config).unwrap());
}

#[test]
fn test_is_daemon_running_no_pid_file() {
    let tmp = TempDir::new().unwrap();
    let config = DaemonConfig {
        pid_file: tmp.path().join("nonexistent.pid"),
        ..DaemonConfig::default()
    };
    assert!(!is_daemon_running(&config).unwrap());
}
