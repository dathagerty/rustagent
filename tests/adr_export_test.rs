use anyhow::Result;
use chrono::Utc;
use rustagent::graph::export::export_adrs;
use rustagent::graph::store::GraphStore;
use rustagent::graph::*;
use std::collections::HashMap;
use std::fs;
use tempfile::TempDir;

mod common;
use common::create_test_goal;

#[tokio::test]
async fn test_ac2_1_export_adrs_creates_numbered_files() -> Result<()> {
    // P1c.AC2.1: export_adrs(project_id, output_dir) generates numbered markdown files (001-xxx.md, 002-xxx.md)
    let (_, graph_store) = common::setup_test_env().await?;
    let temp_dir = TempDir::new()?;

    // Create a goal to contain decisions
    let goal = create_test_goal("ra-goal-1", "proj-1", "Test Goal");
    graph_store.create_node(&goal).await?;

    // Create first decision
    let decision1 = GraphNode {
        id: "ra-goal-1.1".to_string(),
        project_id: "proj-1".to_string(),
        node_type: NodeType::Decision,
        title: "Use Rust".to_string(),
        description: "Choose implementation language".to_string(),
        status: NodeStatus::Decided,
        priority: Some(Priority::High),
        assigned_to: None,
        created_by: None,
        labels: vec![],
        created_at: Utc::now(),
        started_at: None,
        completed_at: None,
        blocked_reason: None,
        metadata: HashMap::new(),
    };
    graph_store.create_node(&decision1).await?;

    // Add a small delay to ensure different created_at times
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    // Create second decision
    let decision2 = GraphNode {
        id: "ra-goal-1.2".to_string(),
        project_id: "proj-1".to_string(),
        node_type: NodeType::Decision,
        title: "PostgreSQL Database".to_string(),
        description: "Choose database system".to_string(),
        status: NodeStatus::Decided,
        priority: Some(Priority::High),
        assigned_to: None,
        created_by: None,
        labels: vec![],
        created_at: Utc::now(),
        started_at: None,
        completed_at: None,
        blocked_reason: None,
        metadata: HashMap::new(),
    };
    graph_store.create_node(&decision2).await?;

    // Export ADRs
    let files = export_adrs(&graph_store, "proj-1", temp_dir.path()).await?;

    // Verify two files were created
    assert_eq!(files.len(), 2);

    // Verify file names start with 001 and 002
    let filenames: Vec<String> = files
        .iter()
        .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .collect();
    filenames.iter().for_each(|f| println!("File: {}", f));

    // At least one file should start with 001 and one with 002
    let has_001 = filenames.iter().any(|f| f.starts_with("001-"));
    let has_002 = filenames.iter().any(|f| f.starts_with("002-"));
    assert!(has_001, "Should have 001- prefixed file");
    assert!(has_002, "Should have 002- prefixed file");

    Ok(())
}

#[tokio::test]
async fn test_ac2_2_export_adrs_contains_all_sections() -> Result<()> {
    // P1c.AC2.2: Each ADR contains Status, Context, Options Considered sections
    let (_, graph_store) = common::setup_test_env().await?;
    let temp_dir = TempDir::new()?;

    // Create a goal
    let goal = create_test_goal("ra-goal-2", "proj-1", "Test Goal");
    graph_store.create_node(&goal).await?;

    // Create a decision
    let decision = GraphNode {
        id: "ra-goal-2.1".to_string(),
        project_id: "proj-1".to_string(),
        node_type: NodeType::Decision,
        title: "Web Framework Choice".to_string(),
        description: "Choosing a web framework for the API server".to_string(),
        status: NodeStatus::Decided,
        priority: Some(Priority::High),
        assigned_to: None,
        created_by: None,
        labels: vec![],
        created_at: Utc::now(),
        started_at: None,
        completed_at: None,
        blocked_reason: None,
        metadata: {
            let mut m = HashMap::new();
            m.insert(
                "outcome".to_string(),
                "Selected Actix-web for performance".to_string(),
            );
            m
        },
    };
    graph_store.create_node(&decision).await?;

    // Create options
    let option1 = GraphNode {
        id: "ra-goal-2.1.1".to_string(),
        project_id: "proj-1".to_string(),
        node_type: NodeType::Option,
        title: "Actix-web".to_string(),
        description: "High-performance async web framework".to_string(),
        status: NodeStatus::Chosen,
        priority: None,
        assigned_to: None,
        created_by: None,
        labels: vec![],
        created_at: Utc::now(),
        started_at: None,
        completed_at: None,
        blocked_reason: None,
        metadata: {
            let mut m = HashMap::new();
            m.insert("pros".to_string(), "- Fast\n- Async".to_string());
            m.insert("cons".to_string(), "- Smaller ecosystem".to_string());
            m
        },
    };
    graph_store.create_node(&option1).await?;

    // Create edges: decision -> option via LeadsTo
    graph_store
        .add_edge(&GraphEdge {
            id: generate_edge_id(),
            edge_type: EdgeType::LeadsTo,
            from_node: "ra-goal-2.1".to_string(),
            to_node: "ra-goal-2.1.1".to_string(),
            label: None,
            created_at: Utc::now(),
        })
        .await?;

    // Edge to mark as chosen with rationale
    graph_store
        .add_edge(&GraphEdge {
            id: generate_edge_id(),
            edge_type: EdgeType::Chosen,
            from_node: "ra-goal-2.1".to_string(),
            to_node: "ra-goal-2.1.1".to_string(),
            label: Some("Best performance characteristics".to_string()),
            created_at: Utc::now(),
        })
        .await?;

    // Export ADRs
    let files = export_adrs(&graph_store, "proj-1", temp_dir.path()).await?;

    // Read the generated file
    assert_eq!(files.len(), 1);
    let content = fs::read_to_string(&files[0])?;

    // Verify all required sections are present
    assert!(content.contains("# ADR"), "Should have ADR title");
    assert!(
        content.contains("**Status:**"),
        "Should have Status section"
    );
    assert!(
        content.contains("## Context"),
        "Should have Context section"
    );
    assert!(
        content.contains("## Options Considered"),
        "Should have Options Considered section"
    );
    assert!(
        content.contains("## Outcome"),
        "Should have Outcome section"
    );
    assert!(
        content.contains("## Related Tasks"),
        "Should have Related Tasks section"
    );

    // Verify content includes option title and status label
    assert!(
        content.contains("Actix-web"),
        "Should reference chosen option"
    );
    assert!(content.contains("CHOSEN"), "Should label option as CHOSEN");

    // Verify rationale is included
    assert!(
        content.contains("Best performance characteristics"),
        "Should include rationale"
    );

    // Verify pros/cons are included
    assert!(content.contains("Pros:"), "Should have Pros section");
    assert!(content.contains("Cons:"), "Should have Cons section");

    Ok(())
}

#[tokio::test]
async fn test_adr_export_with_no_decisions() -> Result<()> {
    // Test that export_adrs works correctly with no decisions
    let (_, graph_store) = common::setup_test_env().await?;
    let temp_dir = TempDir::new()?;

    // Create a goal but no decisions
    let goal = create_test_goal("ra-goal-3", "proj-1", "Test Goal");
    graph_store.create_node(&goal).await?;

    // Export ADRs
    let files = export_adrs(&graph_store, "proj-1", temp_dir.path()).await?;

    // Should produce empty list
    assert_eq!(files.len(), 0);

    Ok(())
}

#[tokio::test]
async fn test_adr_export_slug_generation() -> Result<()> {
    // Test that filenames are properly slugified
    let (_, graph_store) = common::setup_test_env().await?;
    let temp_dir = TempDir::new()?;

    // Create a goal
    let goal = create_test_goal("ra-goal-4", "proj-1", "Test Goal");
    graph_store.create_node(&goal).await?;

    // Create a decision with special characters in title
    let decision = GraphNode {
        id: "ra-goal-4.1".to_string(),
        project_id: "proj-1".to_string(),
        node_type: NodeType::Decision,
        title: "Use TypeScript/React!!!".to_string(),
        description: "Frontend technology choice".to_string(),
        status: NodeStatus::Decided,
        priority: Some(Priority::High),
        assigned_to: None,
        created_by: None,
        labels: vec![],
        created_at: Utc::now(),
        started_at: None,
        completed_at: None,
        blocked_reason: None,
        metadata: HashMap::new(),
    };
    graph_store.create_node(&decision).await?;

    // Export ADRs
    let files = export_adrs(&graph_store, "proj-1", temp_dir.path()).await?;

    assert_eq!(files.len(), 1);
    let filename = files[0].file_name().unwrap().to_string_lossy().to_string();

    // Verify filename is properly slugified
    assert!(filename.starts_with("001-"));
    assert!(
        filename.contains("typescript") || filename.contains("react"),
        "Filename should contain slugified keywords, got: {}",
        filename
    );
    // Should not contain special characters except dash
    assert!(!filename.contains("!"), "Filename should not contain !");
    assert!(!filename.contains("/"), "Filename should not contain /");

    Ok(())
}
