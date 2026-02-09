mod common;

use anyhow::Result;
use common::{create_test_goal, create_test_task, setup_test_env_concurrent};
use rustagent::graph::store::GraphStore;
use rustagent::graph::*;
use std::sync::Arc;

#[tokio::test]
async fn test_concurrent_task_claiming() -> Result<()> {
    let (_db, store) = setup_test_env_concurrent().await?;

    // Create a goal and a task in Ready status
    let goal = create_test_goal("goal-1", "proj-1", "Test Goal");
    let task = create_test_task("task-1", "proj-1", "Claimable Task", NodeStatus::Ready);

    store.create_node(&goal).await?;
    store.create_node(&task).await?;

    // Spawn 10 concurrent tasks all trying to claim the same task
    let mut handles = vec![];
    for i in 0..10 {
        let store_clone = Arc::clone(&store);
        let handle = tokio::spawn(async move {
            store_clone
                .claim_task("task-1", &format!("agent-{}", i))
                .await
        });
        handles.push(handle);
    }

    // Collect results
    let mut results = vec![];
    for handle in handles {
        let result = handle.await??;
        results.push(result);
    }

    // Verify exactly one claim succeeded and the rest failed
    let success_count = results.iter().filter(|&&r| r).count();
    assert_eq!(
        success_count, 1,
        "Expected exactly 1 successful claim, got {}",
        success_count
    );

    let failure_count = results.iter().filter(|&&r| !r).count();
    assert_eq!(
        failure_count, 9,
        "Expected 9 failed claims, got {}",
        failure_count
    );

    // Verify the task now has Claimed status and assigned_to is set to one agent
    let updated_task = store.get_node("task-1").await?;
    assert!(
        updated_task.is_some(),
        "Task should still exist after claiming"
    );

    let updated_task = updated_task.unwrap();
    assert_eq!(
        updated_task.status,
        NodeStatus::Claimed,
        "Task status should be Claimed"
    );
    assert!(
        updated_task.assigned_to.is_some(),
        "Task should have assigned_to set"
    );

    let assigned_agent = updated_task.assigned_to.unwrap();
    assert!(
        assigned_agent.starts_with("agent-"),
        "assigned_to should be an agent ID"
    );

    Ok(())
}

#[tokio::test]
async fn test_concurrent_claim_different_tasks() -> Result<()> {
    let (_db, store) = setup_test_env_concurrent().await?;

    // Create a goal and multiple tasks in Ready status
    let goal = create_test_goal("goal-2", "proj-1", "Test Goal 2");
    store.create_node(&goal).await?;

    // Create 10 Ready tasks
    let mut task_ids = vec![];
    for i in 0..10 {
        let task_id = format!("task-2-{}", i);
        let task = create_test_task(
            &task_id,
            "proj-1",
            &format!("Task {}", i),
            NodeStatus::Ready,
        );
        store.create_node(&task).await?;
        task_ids.push(task_id);
    }

    // Spawn 10 concurrent claim attempts, each for a different task
    let mut handles = vec![];
    for (i, task_id) in task_ids.iter().enumerate() {
        let store_clone = Arc::clone(&store);
        let task_id_clone = task_id.clone();
        let handle = tokio::spawn(async move {
            store_clone
                .claim_task(&task_id_clone, &format!("agent-{}", i))
                .await
        });
        handles.push(handle);
    }

    // Collect results
    let mut results = vec![];
    for handle in handles {
        let result = handle.await??;
        results.push(result);
    }

    // All claims should succeed since each is for a different task
    let success_count = results.iter().filter(|&&r| r).count();
    assert_eq!(
        success_count, 10,
        "Expected all 10 claims to succeed, got {}",
        success_count
    );

    // Verify each task is claimed by its corresponding agent
    for (i, task_id) in task_ids.iter().enumerate() {
        let task = store.get_node(task_id).await?;
        assert!(task.is_some(), "Task {} should exist", task_id);

        let task = task.unwrap();
        assert_eq!(
            task.status,
            NodeStatus::Claimed,
            "Task {} should be Claimed",
            task_id
        );

        let expected_agent = format!("agent-{}", i);
        assert_eq!(
            task.assigned_to.as_ref(),
            Some(&expected_agent),
            "Task {} should be assigned to {}",
            task_id,
            expected_agent
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_claim_non_ready_task_fails() -> Result<()> {
    let (_db, store) = setup_test_env_concurrent().await?;

    // Create a goal and a task that is NOT in Ready status
    let goal = create_test_goal("goal-3", "proj-1", "Test Goal 3");
    let task = create_test_task("task-3", "proj-1", "Non-Ready Task", NodeStatus::Pending);

    store.create_node(&goal).await?;
    store.create_node(&task).await?;

    // Try to claim the task - should fail
    let result = store.claim_task("task-3", "agent-1").await?;
    assert!(!result, "Should not be able to claim a Pending task");

    // Verify the task status is unchanged
    let task = store.get_node("task-3").await?;
    assert!(task.is_some(), "Task should still exist");

    let task = task.unwrap();
    assert_eq!(
        task.status,
        NodeStatus::Pending,
        "Task status should still be Pending"
    );
    assert!(task.assigned_to.is_none(), "Task should not be assigned");

    Ok(())
}

#[tokio::test]
async fn test_concurrent_claim_race_condition() -> Result<()> {
    let (_db, store) = setup_test_env_concurrent().await?;

    // Create a goal and one Ready task
    let goal = create_test_goal("goal-4", "proj-1", "Test Goal 4");
    let task = create_test_task("task-4", "proj-1", "Race Task", NodeStatus::Ready);

    store.create_node(&goal).await?;
    store.create_node(&task).await?;

    // Spawn many more concurrent claim attempts to stress the atomicity
    let num_concurrent = 50;
    let mut handles = vec![];

    for i in 0..num_concurrent {
        let store_clone = Arc::clone(&store);
        let handle = tokio::spawn(async move {
            store_clone
                .claim_task("task-4", &format!("agent-{}", i))
                .await
        });
        handles.push(handle);
    }

    // Collect results
    let mut results = vec![];
    for handle in handles {
        let result = handle.await??;
        results.push(result);
    }

    // Verify exactly one claim succeeded
    let success_count = results.iter().filter(|&&r| r).count();
    assert_eq!(
        success_count, 1,
        "Expected exactly 1 successful claim out of {}, got {}",
        num_concurrent, success_count
    );

    let failure_count = results.iter().filter(|&&r| !r).count();
    assert_eq!(
        failure_count,
        num_concurrent - 1,
        "Expected {} failed claims, got {}",
        num_concurrent - 1,
        failure_count
    );

    // Verify the task has exactly one assigned_to
    let updated_task = store.get_node("task-4").await?;
    assert!(updated_task.is_some(), "Task should exist");

    let updated_task = updated_task.unwrap();
    assert_eq!(updated_task.status, NodeStatus::Claimed);
    assert!(updated_task.assigned_to.is_some());

    // Verify started_at is set (claim_task sets it)
    assert!(
        updated_task.started_at.is_some(),
        "started_at should be set"
    );

    Ok(())
}
