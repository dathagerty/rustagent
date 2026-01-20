use std::process::Command;

#[test]
fn test_cli_help() {
    let output = Command::new("cargo")
        .args(["run", "--", "--help"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("rustagent"));
    assert!(stdout.contains("init"));
    assert!(stdout.contains("plan"));
    assert!(stdout.contains("run"));
}
