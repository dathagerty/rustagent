use std::io::{self, Write};

pub trait PermissionHandler: Send + Sync {
    fn request_permission(&self, request: &PermissionRequest) -> PermissionResult;
}

pub struct PermissionRequest {
    pub resource_type: ResourceType,
    pub action: String,
    pub reason: String,
}

#[derive(Debug, Clone, Copy)]
pub enum ResourceType {
    ShellCommand,
    FilePath,
}

#[derive(Debug)]
pub enum PermissionResult {
    Allow,
    Deny,
    AllowAlways(String),
    Quit,
}

pub struct CliPermissionHandler;

impl PermissionHandler for CliPermissionHandler {
    fn request_permission(&self, request: &PermissionRequest) -> PermissionResult {
        println!("\n⚠️  Permission Required");
        println!("Type: {:?}", request.resource_type);
        println!("Action: {}", request.action);
        println!("Reason: {}", request.reason);
        println!();
        print!("Allow? [y]es / [n]o / [a]lways allow / [q]uit: ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();

        match input.trim().to_lowercase().as_str() {
            "y" | "yes" => PermissionResult::Allow,
            "a" | "always" => {
                // Extract the resource to remember
                let resource = match request.resource_type {
                    ResourceType::ShellCommand => {
                        // Extract base command
                        request.action.split_whitespace().next().unwrap_or("")
                    }
                    ResourceType::FilePath => &request.action,
                };
                PermissionResult::AllowAlways(resource.to_string())
            }
            "q" | "quit" => PermissionResult::Quit,
            _ => PermissionResult::Deny,
        }
    }
}

/// Auto-approve handler for testing
pub struct AutoApproveHandler;

impl PermissionHandler for AutoApproveHandler {
    fn request_permission(&self, _request: &PermissionRequest) -> PermissionResult {
        PermissionResult::Allow
    }
}
