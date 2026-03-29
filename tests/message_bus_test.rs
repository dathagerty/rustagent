use rustagent::message::{MessageBus, TokioMessageBus, WorkerMessage};
use std::path::PathBuf;
use std::time::Duration;

mod common;

/// P2a.AC1.1: All 9 WorkerMessage variants compile with correct fields
#[test]
fn test_worker_message_all_variants() {
    let _progress = WorkerMessage::ProgressReport {
        agent_id: "a1".to_string(),
        turn: 1,
        summary: "working".to_string(),
    };
    let _completed = WorkerMessage::TaskCompleted {
        agent_id: "a1".to_string(),
        task_id: "t1".to_string(),
        summary: "done".to_string(),
    };
    let _blocked = WorkerMessage::TaskBlocked {
        agent_id: "a1".to_string(),
        task_id: "t1".to_string(),
        reason: "stuck".to_string(),
    };
    let _needs_decision = WorkerMessage::NeedsDecision {
        agent_id: "a1".to_string(),
        task_id: "t1".to_string(),
        decision: common::create_test_decision("d1", "proj-1", "Which approach?"),
    };
    let _node_created = WorkerMessage::NodeCreated {
        agent_id: "a1".to_string(),
        parent_id: "ra-a3f8".to_string(),
        node: common::create_test_task(
            "t2",
            "proj-1",
            "subtask",
            rustagent::graph::NodeStatus::Ready,
        ),
    };
    let _cancel = WorkerMessage::Cancel {
        reason: "timeout".to_string(),
    };
    let _additional = WorkerMessage::AdditionalContext {
        content: "extra info".to_string(),
    };
    let _review_req = WorkerMessage::ReviewRequest {
        work_package_id: "wp-12345678".to_string(),
        changed_files: vec![PathBuf::from("src/main.rs")],
    };
    let _review_fb = WorkerMessage::ReviewFeedback {
        approved: true,
        comments: vec!["LGTM".to_string()],
    };
}

/// P2a.AC1.3: WorkerMessage is Clone + Debug
#[test]
fn test_worker_message_clone_debug() {
    let msg = WorkerMessage::ProgressReport {
        agent_id: "a1".to_string(),
        turn: 5,
        summary: "halfway".to_string(),
    };
    let cloned = msg.clone();
    let debug = format!("{:?}", cloned);
    assert!(debug.contains("ProgressReport"));
    assert!(debug.contains("halfway"));
}

/// P2a.AC2.2: Send a message to a specific agent's mpsc channel
#[tokio::test]
async fn test_targeted_send() {
    let bus = TokioMessageBus::new(64, 32);
    let mut rx = bus.subscribe(&"a1".to_string());

    bus.send(
        &"a1".to_string(),
        WorkerMessage::Cancel {
            reason: "test".to_string(),
        },
    )
    .await
    .unwrap();

    let msg = tokio::time::timeout(Duration::from_millis(100), rx.recv())
        .await
        .unwrap()
        .unwrap();

    match msg {
        WorkerMessage::Cancel { reason } => assert_eq!(reason, "test"),
        other => panic!("expected Cancel, got {:?}", other),
    }
}

/// P2a.AC2.3 + P2a.AC3.2: Broadcast delivers to all subscribers
#[tokio::test]
async fn test_broadcast_to_all() {
    let bus = TokioMessageBus::new(64, 32);
    let mut rx1 = bus.subscribe(&"a1".to_string());
    let mut rx2 = bus.subscribe(&"a2".to_string());

    // Give forwarding tasks time to start
    tokio::task::yield_now().await;

    bus.broadcast(WorkerMessage::Cancel {
        reason: "shutdown".to_string(),
    })
    .await
    .unwrap();

    let msg1 = tokio::time::timeout(Duration::from_millis(100), rx1.recv())
        .await
        .unwrap()
        .unwrap();
    let msg2 = tokio::time::timeout(Duration::from_millis(100), rx2.recv())
        .await
        .unwrap()
        .unwrap();

    match msg1 {
        WorkerMessage::Cancel { reason } => assert_eq!(reason, "shutdown"),
        other => panic!("expected Cancel, got {:?}", other),
    }
    match msg2 {
        WorkerMessage::Cancel { reason } => assert_eq!(reason, "shutdown"),
        other => panic!("expected Cancel, got {:?}", other),
    }
}

/// P2a.AC3.3: Targeted message reaches only the intended agent
#[tokio::test]
async fn test_targeted_message_exclusivity() {
    let bus = TokioMessageBus::new(64, 32);
    let mut rx1 = bus.subscribe(&"a1".to_string());
    let mut rx2 = bus.subscribe(&"a2".to_string());

    bus.send(
        &"a1".to_string(),
        WorkerMessage::Cancel {
            reason: "for a1 only".to_string(),
        },
    )
    .await
    .unwrap();

    // a1 should receive it
    let msg = tokio::time::timeout(Duration::from_millis(100), rx1.recv())
        .await
        .unwrap()
        .unwrap();
    match msg {
        WorkerMessage::Cancel { reason } => assert_eq!(reason, "for a1 only"),
        other => panic!("expected Cancel, got {:?}", other),
    }

    // a2 should NOT receive it (timeout)
    let result = tokio::time::timeout(Duration::from_millis(50), rx2.recv()).await;
    assert!(
        result.is_err(),
        "a2 should not receive a1's targeted message"
    );
}

/// P2a.AC3.4: Messages sent before subscription are not received
#[tokio::test]
async fn test_no_retroactive_messages() {
    let bus = TokioMessageBus::new(64, 32);

    // Broadcast before subscribing
    bus.broadcast(WorkerMessage::Cancel {
        reason: "early".to_string(),
    })
    .await
    .unwrap();

    // Now subscribe
    let mut rx = bus.subscribe(&"a1".to_string());

    // Should not receive the earlier broadcast
    let result = tokio::time::timeout(Duration::from_millis(50), rx.recv()).await;
    assert!(
        result.is_err(),
        "should not receive messages sent before subscription"
    );
}

/// P2a.AC3.5: Dropping a subscriber does not crash the bus
#[tokio::test]
async fn test_dropped_subscriber_no_crash() {
    let bus = TokioMessageBus::new(64, 32);
    let rx = bus.subscribe(&"a1".to_string());

    // Give forwarding task time to start
    tokio::task::yield_now().await;

    // Drop the receiver
    drop(rx);

    // Small delay for forwarding task to notice the drop
    tokio::time::sleep(Duration::from_millis(10)).await;

    // Broadcast should not panic
    bus.broadcast(WorkerMessage::Cancel {
        reason: "after drop".to_string(),
    })
    .await
    .unwrap();
}

/// remove_subscriber: After removal, send returns error
#[tokio::test]
async fn test_remove_subscriber() {
    let bus = TokioMessageBus::new(64, 32);
    let _rx = bus.subscribe(&"a1".to_string());

    bus.remove_subscriber(&"a1".to_string());

    let result = bus
        .send(
            &"a1".to_string(),
            WorkerMessage::Cancel {
                reason: "test".to_string(),
            },
        )
        .await;
    assert!(result.is_err(), "send to removed subscriber should fail");
}

/// Send to non-existent agent returns error
#[tokio::test]
async fn test_send_to_nonexistent_agent() {
    let bus = TokioMessageBus::new(64, 32);
    let result = bus
        .send(
            &"nonexistent".to_string(),
            WorkerMessage::Cancel {
                reason: "test".to_string(),
            },
        )
        .await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not subscribed"));
}
