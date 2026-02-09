mod common;

use anyhow::Result;
use chrono::Utc;
use common::{create_test_goal, create_test_task_with_priority, setup_test_env};
use rustagent::graph::store::GraphStore;
use rustagent::graph::*;
use std::time::Duration;
use tokio::time::sleep;

/// P1b.AC4.1: Task moves from Pending to Ready when all DependsOn targets are Completed
#[tokio::test]
async fn test_task_pending_to_ready_when_deps_complete() -> Result<()> {
    let (_db, store) = setup_test_env().await?;

    // Create a goal and two tasks
    let goal = create_test_goal("ra-a1b2", "proj-1", "Test Goal");
    let task_a = create_test_task_with_priority(
        "ra-a1b2.1",
        "proj-1",
        "Task A",
        NodeStatus::Pending,
        Some(Priority::Medium),
    );
    let task_b = create_test_task_with_priority(
        "ra-a1b2.2",
        "proj-1",
        "Task B",
        NodeStatus::Pending,
        Some(Priority::Medium),
    );

    store.create_node(&goal).await?;
    store.create_node(&task_a).await?;
    store.create_node(&task_b).await?;

    // Create a DependsOn edge: Task B depends on Task A
    let edge = GraphEdge {
        id: generate_edge_id(),
        edge_type: EdgeType::DependsOn,
        from_node: "ra-a1b2.2".to_string(),
        to_node: "ra-a1b2.1".to_string(),
        label: None,
        created_at: Utc::now(),
    };
    store.add_edge(&edge).await?;

    // Initially, Task B should still be Pending
    let task_b_before = store.get_node("ra-a1b2.2").await?;
    assert!(task_b_before.is_some());
    assert_eq!(task_b_before.unwrap().status, NodeStatus::Pending);

    // Complete Task A
    store
        .update_node("ra-a1b2.1", Some(NodeStatus::Completed), None, None, None)
        .await?;

    // Now Task B should be Ready (automatically promoted by the status transition hook)
    let task_b_after = store.get_node("ra-a1b2.2").await?;
    assert!(task_b_after.is_some());
    let task_b_node = task_b_after.unwrap();
    assert_eq!(task_b_node.status, NodeStatus::Ready);

    Ok(())
}

/// P1b.AC4.2: get_ready_tasks returns only tasks in Ready status with all deps satisfied
#[tokio::test]
async fn test_get_ready_tasks_filters_correctly() -> Result<()> {
    let (_db, store) = setup_test_env().await?;

    // Create a goal
    let goal = create_test_goal("ra-a1b2", "proj-1", "Test Goal");
    store.create_node(&goal).await?;

    // Create three tasks: one Ready, one Pending (with unmet deps), one Completed
    let task_ready = create_test_task_with_priority(
        "ra-a1b2.1",
        "proj-1",
        "Task Ready",
        NodeStatus::Ready,
        Some(Priority::Medium),
    );
    let task_pending = create_test_task_with_priority(
        "ra-a1b2.2",
        "proj-1",
        "Task Pending",
        NodeStatus::Pending,
        Some(Priority::Medium),
    );
    let task_completed = create_test_task_with_priority(
        "ra-a1b2.3",
        "proj-1",
        "Task Completed",
        NodeStatus::Completed,
        Some(Priority::Medium),
    );

    store.create_node(&task_ready).await?;
    store.create_node(&task_pending).await?;
    store.create_node(&task_completed).await?;

    // Create a DependsOn edge: Task Pending depends on Task Completed
    let edge = GraphEdge {
        id: generate_edge_id(),
        edge_type: EdgeType::DependsOn,
        from_node: "ra-a1b2.2".to_string(),
        to_node: "ra-a1b2.3".to_string(),
        label: None,
        created_at: Utc::now(),
    };
    store.add_edge(&edge).await?;

    // get_ready_tasks should return only the one Ready task
    let ready_tasks = store.get_ready_tasks("ra-a1b2").await?;
    assert_eq!(ready_tasks.len(), 1);
    assert_eq!(ready_tasks[0].id, "ra-a1b2.1");
    assert_eq!(ready_tasks[0].status, NodeStatus::Ready);

    Ok(())
}

/// P1b.AC4.3: get_next_task returns highest-priority Ready task, breaking ties by downstream unblock count
#[tokio::test]
async fn test_get_next_task_priority_and_downstream() -> Result<()> {
    let (_db, store) = setup_test_env().await?;

    // Create a goal
    let goal = create_test_goal("ra-a1b2", "proj-1", "Test Goal");
    store.create_node(&goal).await?;

    // Create two ready tasks: one High priority (blocking 3 tasks), one Critical priority (blocking 0)
    let task_high_priority = create_test_task_with_priority(
        "ra-a1b2.1",
        "proj-1",
        "High Priority",
        NodeStatus::Ready,
        Some(Priority::High),
    );
    let task_critical_priority = create_test_task_with_priority(
        "ra-a1b2.2",
        "proj-1",
        "Critical Priority",
        NodeStatus::Ready,
        Some(Priority::Critical),
    );

    store.create_node(&task_high_priority).await?;
    store.create_node(&task_critical_priority).await?;

    // Create 3 more tasks that depend on the High priority task
    let dependent1 = create_test_task_with_priority(
        "ra-a1b2.3",
        "proj-1",
        "Dependent 1",
        NodeStatus::Pending,
        Some(Priority::Medium),
    );
    let dependent2 = create_test_task_with_priority(
        "ra-a1b2.4",
        "proj-1",
        "Dependent 2",
        NodeStatus::Pending,
        Some(Priority::Medium),
    );
    let dependent3 = create_test_task_with_priority(
        "ra-a1b2.5",
        "proj-1",
        "Dependent 3",
        NodeStatus::Pending,
        Some(Priority::Medium),
    );

    store.create_node(&dependent1).await?;
    store.create_node(&dependent2).await?;
    store.create_node(&dependent3).await?;

    // Create DependsOn edges from the three dependents to the high priority task
    for i in 3..=5 {
        let edge = GraphEdge {
            id: generate_edge_id(),
            edge_type: EdgeType::DependsOn,
            from_node: format!("ra-a1b2.{}", i),
            to_node: "ra-a1b2.1".to_string(),
            label: None,
            created_at: Utc::now(),
        };
        store.add_edge(&edge).await?;
    }

    // get_next_task should return the Critical priority task (priority wins over downstream count)
    let next_task = store.get_next_task("ra-a1b2").await?;
    assert!(next_task.is_some());
    let task = next_task.unwrap();
    assert_eq!(task.id, "ra-a1b2.2");
    assert_eq!(task.priority, Some(Priority::Critical));

    Ok(())
}

/// P1b.AC4.3 variant: When priorities are equal, downstream count should be the tiebreaker
/// This test creates tasks in reverse order (B before A) to ensure that created_at ordering
/// would pick the wrong task without the downstream count logic.
#[tokio::test]
async fn test_get_next_task_tiebreak_by_downstream() -> Result<()> {
    let (_db, store) = setup_test_env().await?;

    // Create a goal
    let goal = create_test_goal("ra-a1b2", "proj-1", "Test Goal");
    store.create_node(&goal).await?;

    // Create Task B FIRST (so it has an earlier created_at)
    let task_b = create_test_task_with_priority(
        "ra-a1b2.2",
        "proj-1",
        "Task B",
        NodeStatus::Ready,
        Some(Priority::High),
    );
    store.create_node(&task_b).await?;

    // Small delay to ensure Task A has a later created_at
    sleep(Duration::from_millis(10)).await;

    // Create Task A SECOND (so it has a later created_at)
    // Without downstream count logic, ordering by created_at would pick B
    let task_a = create_test_task_with_priority(
        "ra-a1b2.1",
        "proj-1",
        "Task A",
        NodeStatus::Ready,
        Some(Priority::High),
    );
    store.create_node(&task_a).await?;

    // Create 3 tasks that depend on Task A (higher downstream count)
    let dep_a1 = create_test_task_with_priority(
        "ra-a1b2.3",
        "proj-1",
        "Dep A1",
        NodeStatus::Pending,
        Some(Priority::Medium),
    );
    let dep_a2 = create_test_task_with_priority(
        "ra-a1b2.4",
        "proj-1",
        "Dep A2",
        NodeStatus::Pending,
        Some(Priority::Medium),
    );
    let dep_a3 = create_test_task_with_priority(
        "ra-a1b2.5",
        "proj-1",
        "Dep A3",
        NodeStatus::Pending,
        Some(Priority::Medium),
    );

    store.create_node(&dep_a1).await?;
    store.create_node(&dep_a2).await?;
    store.create_node(&dep_a3).await?;

    // Create edges: 3 tasks depend on A, 0 on B
    for i in 3..=5 {
        let edge = GraphEdge {
            id: generate_edge_id(),
            edge_type: EdgeType::DependsOn,
            from_node: format!("ra-a1b2.{}", i),
            to_node: "ra-a1b2.1".to_string(),
            label: None,
            created_at: Utc::now(),
        };
        store.add_edge(&edge).await?;
    }

    // get_next_task should return Task A (higher downstream count), not Task B (earlier created_at)
    let next_task = store.get_next_task("ra-a1b2").await?;
    assert!(next_task.is_some());
    let task = next_task.unwrap();
    assert_eq!(
        task.id, "ra-a1b2.1",
        "Expected Task A with higher downstream count, not Task B with earlier created_at"
    );

    Ok(())
}
