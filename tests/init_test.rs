use std::process::Command;
use tempfile::TempDir;

#[test]
fn test_init_creates_spec_directory() {
    let temp = TempDir::new().unwrap();
    let spec_dir = temp.path().join("specs");

    let exe = env!("CARGO_BIN_EXE_rustagent");

    let output = Command::new(exe)
        .args(["init", "--spec-dir", spec_dir.to_str().unwrap()])
        .current_dir(temp.path())
        .output()
        .unwrap();

    assert!(output.status.success(), "Command failed: {:?}", output);
    assert!(spec_dir.exists());
    assert!(spec_dir.is_dir());
}

#[test]
fn test_init_creates_config_template() {
    let temp = TempDir::new().unwrap();
    let spec_dir = temp.path().join("specs");

    let exe = env!("CARGO_BIN_EXE_rustagent");

    let output = Command::new(exe)
        .args(["init", "--spec-dir", spec_dir.to_str().unwrap()])
        .current_dir(temp.path())
        .output()
        .unwrap();

    assert!(output.status.success(), "Command failed: {:?}", output);

    let config_path = temp.path().join("rustagent.toml");
    assert!(config_path.exists(), "Config file was not created");
}

#[test]
fn test_init_idempotent() {
    let temp = TempDir::new().unwrap();
    let spec_dir = temp.path().join("specs");

    let exe = env!("CARGO_BIN_EXE_rustagent");

    Command::new(exe)
        .args(["init", "--spec-dir", spec_dir.to_str().unwrap()])
        .current_dir(temp.path())
        .output()
        .unwrap();

    let output = Command::new(exe)
        .args(["init", "--spec-dir", spec_dir.to_str().unwrap()])
        .current_dir(temp.path())
        .output()
        .unwrap();

    assert!(output.status.success(), "Second init failed: {:?}", output);
}
