use crate::security::SecurityValidator;
use crate::security::permission::PermissionHandler;
use crate::tools::ToolRegistry;
use crate::tools::file::{ListFilesTool, ReadFileTool, WriteFileTool};
use crate::tools::shell::RunCommandTool;
use crate::tools::signal::SignalTool;
use std::sync::Arc;

pub fn create_default_registry(
    validator: Arc<SecurityValidator>,
    permission_handler: Arc<dyn PermissionHandler>,
) -> ToolRegistry {
    let registry = ToolRegistry::new();

    registry.register(Arc::new(ReadFileTool::new(
        validator.clone(),
        permission_handler.clone(),
    )));
    registry.register(Arc::new(WriteFileTool::new(
        validator.clone(),
        permission_handler.clone(),
    )));
    registry.register(Arc::new(ListFilesTool::new(
        validator.clone(),
        permission_handler.clone(),
    )));
    registry.register(Arc::new(RunCommandTool::new(
        validator.clone(),
        permission_handler.clone(),
    )));
    registry.register(Arc::new(SignalTool::new()));

    registry
}
