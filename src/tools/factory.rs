use crate::graph::store::GraphStore;
use crate::security::SecurityValidator;
use crate::security::permission::PermissionHandler;
use crate::tools::ToolRegistry;
use crate::tools::file::{ListFilesTool, ReadFileTool, WriteFileTool};
use crate::tools::graph_tools::{
    AddEdgeTool, ChooseOptionTool, ClaimTaskTool, CreateNodeTool, LogDecisionTool, QueryNodesTool,
    RecordObservationTool, RecordOutcomeTool, RevisitTool, SearchNodesTool, UpdateNodeTool,
};
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

/// Create a v2 registry for agent runtime with graph tools registered
pub fn create_v2_registry(
    validator: Arc<SecurityValidator>,
    permission_handler: Arc<dyn PermissionHandler>,
    graph_store: Arc<dyn GraphStore>,
) -> ToolRegistry {
    let registry = create_default_registry(validator, permission_handler);

    // Register graph tools
    registry.register(Arc::new(CreateNodeTool::new(graph_store.clone())));
    registry.register(Arc::new(UpdateNodeTool::new(graph_store.clone())));
    registry.register(Arc::new(AddEdgeTool::new(graph_store.clone())));
    registry.register(Arc::new(QueryNodesTool::new(graph_store.clone())));
    registry.register(Arc::new(SearchNodesTool::new(graph_store.clone())));
    registry.register(Arc::new(ClaimTaskTool::new(graph_store.clone())));
    registry.register(Arc::new(LogDecisionTool::new(graph_store.clone())));
    registry.register(Arc::new(ChooseOptionTool::new(graph_store.clone())));
    registry.register(Arc::new(RecordOutcomeTool::new(graph_store.clone())));
    registry.register(Arc::new(RecordObservationTool::new(graph_store.clone())));
    registry.register(Arc::new(RevisitTool::new(graph_store)));

    registry
}
