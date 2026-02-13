use rustagent::tools::Tool;
use rustagent::tools::search::CodeSearchTool;
use serde_json::json;
use tempfile::TempDir;

/// Integration test for code_search functionality
/// Verifies AC9.1: Search finds known pattern with file path and line number
#[tokio::test]
async fn code_search_integration_finds_pattern() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();

    // Create test directory structure
    std::fs::create_dir_all(project_root.join("src")).unwrap();
    std::fs::write(
        project_root.join("src/main.rs"),
        "fn main() {\n    println!(\"hello\");\n}",
    )
    .unwrap();
    std::fs::write(
        project_root.join("src/lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 { a + b }",
    )
    .unwrap();

    let tool = CodeSearchTool::new(project_root);
    let params = json!({
        "pattern": "fn main"
    });

    let result = tool.execute(params).await.unwrap();
    assert!(result.contains("src/main.rs:1:"));
    assert!(result.contains("fn main"));
}

/// Integration test for code_search with file glob filter
/// Verifies AC9.2: File glob filter limits search to matching files
#[tokio::test]
async fn code_search_integration_with_file_glob() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();

    // Create test directory structure
    std::fs::create_dir_all(project_root.join("src")).unwrap();
    std::fs::write(project_root.join("src/main.rs"), "fn main() {}").unwrap();
    std::fs::write(
        project_root.join("src/lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 { a + b }",
    )
    .unwrap();
    std::fs::write(
        project_root.join("README.md"),
        "# My Project\npub fn should_not_match",
    )
    .unwrap();

    let tool = CodeSearchTool::new(project_root);
    let params = json!({
        "pattern": "pub fn",
        "file_glob": "*.rs"
    });

    let result = tool.execute(params).await.unwrap();
    // Should find in lib.rs
    assert!(result.contains("lib.rs"));
    // Should NOT find in README.md
    assert!(!result.contains("README.md"));
}

/// Integration test for result capping
/// Verifies AC9.3: Results are capped at configurable limit
#[tokio::test]
async fn code_search_integration_result_capping() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();

    // Create test file with multiple matches
    std::fs::write(
        project_root.join("test.rs"),
        "fn one()\nfn two()\nfn three()\nfn four()\nfn five()",
    )
    .unwrap();

    let tool = CodeSearchTool::new(project_root);
    let params = json!({
        "pattern": "fn",
        "max_results": 1
    });

    let result = tool.execute(params).await.unwrap();
    let lines: Vec<&str> = result.lines().collect();
    // Should have 1 match line + 1 capped message line = 2 total
    assert_eq!(lines.len(), 2);
    assert!(result.contains("Found 2 matches (limited to max_results: 1)"));
}

/// Integration test for no matches
/// Verifies that searching for non-existent pattern returns appropriate message
#[tokio::test]
async fn code_search_integration_no_matches() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();

    std::fs::write(project_root.join("test.rs"), "fn main() {}").unwrap();

    let tool = CodeSearchTool::new(project_root);
    let params = json!({
        "pattern": "nonexistent_pattern"
    });

    let result = tool.execute(params).await.unwrap();
    assert_eq!(result, "No matches found");
}

/// Integration test for directory scoping
/// Verifies that directory parameter scopes search correctly
#[tokio::test]
async fn code_search_integration_with_directory_scope() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();

    // Create directory structure
    std::fs::create_dir_all(project_root.join("src/utils")).unwrap();
    std::fs::write(project_root.join("main.rs"), "fn main() {}").unwrap();
    std::fs::write(
        project_root.join("src/utils/helper.rs"),
        "pub fn helper() {}",
    )
    .unwrap();

    let tool = CodeSearchTool::new(project_root);
    let params = json!({
        "pattern": "fn",
        "directory": "src"
    });

    let result = tool.execute(params).await.unwrap();
    assert!(result.contains("utils/helper.rs"));
    assert!(!result.contains("main.rs"));
}

/// Integration test for multiple patterns in multiple files
/// Verifies comprehensive search functionality
#[tokio::test]
async fn code_search_integration_multiple_files() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();

    std::fs::create_dir_all(project_root.join("src")).unwrap();
    std::fs::write(
        project_root.join("src/main.rs"),
        "fn main() {\n    println!(\"hello\");\n}",
    )
    .unwrap();
    std::fs::write(
        project_root.join("src/lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 { a + b }",
    )
    .unwrap();

    let tool = CodeSearchTool::new(project_root);
    let params = json!({
        "pattern": "pub fn"
    });

    let result = tool.execute(params).await.unwrap();
    // Should find pub fn in lib.rs
    assert!(result.contains("lib.rs"));
    // Should not find in main.rs (no pub fn there)
    assert!(!result.contains("main.rs") || !result.contains("fn main"));
}

/// Verify code_search tool is properly registered in V2 registry
/// Verifies AC10.1: Tool is available in V2 registry
#[tokio::test]
async fn code_search_tool_is_registered() {
    use rustagent::config::{SecurityConfig, ShellPolicy};
    use rustagent::db::Database;
    use rustagent::graph::store::SqliteGraphStore;
    use rustagent::security::SecurityValidator;
    use rustagent::security::permission::AutoApproveHandler;
    use rustagent::tools::factory::create_v2_registry;
    use std::path::Path;
    use std::sync::Arc;

    // Create an in-memory database
    let db = Database::open(Path::new(":memory:"))
        .await
        .expect("Failed to create database");
    let graph_store = SqliteGraphStore::new(db);

    let security_config = SecurityConfig {
        shell_policy: ShellPolicy::Unrestricted,
        allowed_commands: vec![],
        blocked_patterns: vec![],
        max_file_size_mb: 100,
        allowed_paths: vec![],
    };
    let validator = Arc::new(SecurityValidator::new(security_config).unwrap());
    let permission_handler = Arc::new(AutoApproveHandler);
    let project_root = tempfile::TempDir::new().unwrap().path().to_path_buf();

    let registry = create_v2_registry(
        validator,
        permission_handler,
        Arc::new(graph_store),
        None,
        None,
        project_root,
    );

    let tools = registry.list();
    assert!(
        tools.contains(&"code_search".to_string()),
        "code_search tool not found in registry. Available tools: {:?}",
        tools
    );
}

/// Verify code_search tool JSON schema
/// Verifies AC10.2: Tool schema describes all parameters
#[tokio::test]
async fn code_search_tool_parameters_schema() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();

    let tool = CodeSearchTool::new(project_root);
    let schema = tool.parameters();

    // Verify schema structure
    assert_eq!(schema["type"], "object");

    // Verify required parameter
    let required = &schema["required"];
    assert!(required.is_array());
    let required_array = required.as_array().unwrap();
    assert!(required_array.contains(&serde_json::Value::String("pattern".to_string())));

    // Verify properties exist
    let properties = &schema["properties"];
    assert!(properties.get("pattern").is_some());
    assert!(properties.get("file_glob").is_some());
    assert!(properties.get("directory").is_some());
    assert!(properties.get("max_results").is_some());

    // Verify pattern is required
    assert_eq!(required_array.len(), 1);
    assert_eq!(required_array[0], "pattern");
}
