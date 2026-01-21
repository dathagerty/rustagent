use rustagent::security::permission::{
    AutoApproveHandler, PermissionHandler, PermissionRequest, PermissionResult, ResourceType,
};

#[test]
fn test_auto_approve_handler() {
    let handler = AutoApproveHandler;

    let request = PermissionRequest {
        resource_type: ResourceType::ShellCommand,
        action: "git status".to_string(),
        reason: "Not in allowlist".to_string(),
    };

    match handler.request_permission(&request) {
        PermissionResult::Allow => {}
        _ => panic!("Expected auto-approve"),
    }
}
