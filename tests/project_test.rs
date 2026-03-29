use rustagent::db::Database;
use rustagent::project::ProjectStore;
use std::path::Path;

/// Test: P1a.AC3.1 - add() creates project with ra- prefixed ID
#[tokio::test]
async fn test_add_creates_project_with_id() {
    let db = Database::open_in_memory()
        .await
        .expect("failed to open in-memory database");
    let store = ProjectStore::new(db);

    let project = store
        .add("my-api", Path::new("/tmp/test"))
        .await
        .expect("failed to add project");

    assert!(project.id.starts_with("ra-"));
    assert_eq!(project.id.len(), 7); // "ra-" + 4 hex chars
    assert_eq!(project.name, "my-api");
    assert_eq!(project.path, Path::new("/tmp/test"));
}

/// Test: P1a.AC3.5 - Adding duplicate project name returns error
#[tokio::test]
async fn test_add_duplicate_name_fails() {
    let db = Database::open_in_memory()
        .await
        .expect("failed to open in-memory database");
    let store = ProjectStore::new(db);

    let _ = store
        .add("my-api", Path::new("/tmp/test1"))
        .await
        .expect("first add should succeed");

    let result = store.add("my-api", Path::new("/tmp/test2")).await;

    assert!(result.is_err(), "adding duplicate name should fail");
}

/// Test: P1a.AC3.2 - list() returns all projects ordered by name
#[tokio::test]
async fn test_list_returns_all_projects_ordered() {
    let db = Database::open_in_memory()
        .await
        .expect("failed to open in-memory database");
    let store = ProjectStore::new(db);

    // Add projects in reverse name order
    let _ = store
        .add("zebra-proj", Path::new("/tmp/zebra"))
        .await
        .expect("failed to add zebra project");
    let _ = store
        .add("alpha-proj", Path::new("/tmp/alpha"))
        .await
        .expect("failed to add alpha project");
    let _ = store
        .add("beta-proj", Path::new("/tmp/beta"))
        .await
        .expect("failed to add beta project");

    let projects = store.list().await.expect("failed to list projects");

    assert_eq!(projects.len(), 3);
    assert_eq!(projects[0].name, "alpha-proj");
    assert_eq!(projects[1].name, "beta-proj");
    assert_eq!(projects[2].name, "zebra-proj");
}

/// Test: P1a.AC3.3 - get_by_name() returns project details
#[tokio::test]
async fn test_get_by_name_returns_project() {
    let db = Database::open_in_memory()
        .await
        .expect("failed to open in-memory database");
    let store = ProjectStore::new(db);

    let added = store
        .add("my-api", Path::new("/tmp/test"))
        .await
        .expect("failed to add project");

    let retrieved = store
        .get_by_name("my-api")
        .await
        .expect("failed to get project")
        .expect("project should exist");

    assert_eq!(retrieved.id, added.id);
    assert_eq!(retrieved.name, "my-api");
    assert_eq!(retrieved.path, Path::new("/tmp/test"));
}

/// Test: get_by_name() returns None for non-existent project
#[tokio::test]
async fn test_get_by_name_not_found() {
    let db = Database::open_in_memory()
        .await
        .expect("failed to open in-memory database");
    let store = ProjectStore::new(db);

    let result = store
        .get_by_name("nonexistent")
        .await
        .expect("query should succeed");

    assert!(result.is_none());
}

/// Test: P1a.AC3.4 - remove() deletes a project
#[tokio::test]
async fn test_remove_deletes_project() {
    let db = Database::open_in_memory()
        .await
        .expect("failed to open in-memory database");
    let store = ProjectStore::new(db);

    let _ = store
        .add("my-api", Path::new("/tmp/test"))
        .await
        .expect("failed to add project");

    // Verify it exists
    let before = store
        .get_by_name("my-api")
        .await
        .expect("query should succeed");
    assert!(before.is_some());

    // Remove it
    let deleted = store
        .remove("my-api")
        .await
        .expect("failed to remove project");
    assert!(deleted);

    // Verify it's gone
    let after = store
        .get_by_name("my-api")
        .await
        .expect("query should succeed");
    assert!(after.is_none());
}

/// Test: remove() returns false if project doesn't exist
#[tokio::test]
async fn test_remove_nonexistent_returns_false() {
    let db = Database::open_in_memory()
        .await
        .expect("failed to open in-memory database");
    let store = ProjectStore::new(db);

    let deleted = store
        .remove("nonexistent")
        .await
        .expect("remove should not fail");

    assert!(!deleted);
}

/// Test: P1a.AC3.6 - get_by_path() resolves project by path
#[tokio::test]
async fn test_get_by_path_resolves_project() {
    let db = Database::open_in_memory()
        .await
        .expect("failed to open in-memory database");
    let store = ProjectStore::new(db);

    let added = store
        .add("my-proj", Path::new("/tmp/test-proj"))
        .await
        .expect("failed to add project");

    let retrieved = store
        .get_by_path(Path::new("/tmp/test-proj"))
        .await
        .expect("failed to get project by path")
        .expect("project should be found");

    assert_eq!(retrieved.id, added.id);
    assert_eq!(retrieved.name, "my-proj");
}

/// Test: get_by_path() returns None for non-matching path
#[tokio::test]
async fn test_get_by_path_not_found() {
    let db = Database::open_in_memory()
        .await
        .expect("failed to open in-memory database");
    let store = ProjectStore::new(db);

    let _ = store
        .add("my-proj", Path::new("/tmp/test-proj"))
        .await
        .expect("failed to add project");

    let result = store
        .get_by_path(Path::new("/tmp/other-proj"))
        .await
        .expect("query should succeed");

    assert!(result.is_none());
}
