use anyhow::Result;
use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::db::Database;

/// Represents a registered project in the system
#[derive(Clone, Debug)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub registered_at: DateTime<Utc>,
    pub config_overrides: Option<String>,
    pub metadata: String,
}

/// Project storage and retrieval operations
#[derive(Clone)]
pub struct ProjectStore {
    db: Database,
}

impl ProjectStore {
    /// Create a new ProjectStore
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Register a new project
    ///
    /// Generates a unique ID with prefix "ra-" followed by 4 hex characters.
    /// Uses BEGIN IMMEDIATE transaction for write safety.
    pub async fn add(&self, name: &str, path: &Path) -> Result<Project> {
        let db = self.db.clone();
        let name = name.to_string();
        let path = path.to_path_buf();

        let result = db
            .connection()
            .call(move |conn| {
                let tx =
                    conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

                let id = generate_project_id();
                let now = Utc::now();
                let registered_at = now.to_rfc3339();

                tx.execute(
                    "INSERT INTO projects (id, name, path, registered_at, metadata)
                     VALUES (?, ?, ?, ?, ?)",
                    rusqlite::params![&id, &name, path.to_string_lossy(), &registered_at, "{}"],
                )?;

                tx.commit()?;

                Ok(Project {
                    id,
                    name,
                    path,
                    registered_at: now,
                    config_overrides: None,
                    metadata: "{}".to_string(),
                })
            })
            .await;

        result.map_err(|e| anyhow::anyhow!(e))
    }

    /// List all projects ordered by name
    pub async fn list(&self) -> Result<Vec<Project>> {
        let db = self.db.clone();

        let result = db
            .connection()
            .call(|conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, name, path, registered_at, config_overrides, metadata
                     FROM projects
                     ORDER BY name",
                )?;

                let projects = stmt.query_map([], |row| {
                    let registered_at_str: String = row.get(3)?;
                    let registered_at = DateTime::parse_from_rfc3339(&registered_at_str)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now());

                    Ok(Project {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        path: PathBuf::from(row.get::<_, String>(2)?),
                        registered_at,
                        config_overrides: row.get(4)?,
                        metadata: row.get(5)?,
                    })
                })?;

                let mut projects_vec = Vec::new();
                for project in projects {
                    projects_vec.push(project?);
                }
                Ok(projects_vec)
            })
            .await;

        result.map_err(|e| anyhow::anyhow!(e))
    }

    /// Get a project by name
    pub async fn get_by_name(&self, name: &str) -> Result<Option<Project>> {
        let db = self.db.clone();
        let name = name.to_string();

        let result = db
            .connection()
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, name, path, registered_at, config_overrides, metadata
                     FROM projects
                     WHERE name = ?",
                )?;

                let project = stmt.query_row([&name], |row| {
                    let registered_at_str: String = row.get(3)?;
                    let registered_at = DateTime::parse_from_rfc3339(&registered_at_str)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now());

                    Ok(Project {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        path: PathBuf::from(row.get::<_, String>(2)?),
                        registered_at,
                        config_overrides: row.get(4)?,
                        metadata: row.get(5)?,
                    })
                });

                match project {
                    Ok(p) => Ok(Some(p)),
                    Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                    Err(e) => Err(tokio_rusqlite::Error::Rusqlite(e)),
                }
            })
            .await;

        result.map_err(|e| anyhow::anyhow!(e))
    }

    /// Get a project by ID
    pub async fn get_by_id(&self, id: &str) -> Result<Option<Project>> {
        let db = self.db.clone();
        let id = id.to_string();

        let result = db
            .connection()
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, name, path, registered_at, config_overrides, metadata
                     FROM projects
                     WHERE id = ?",
                )?;

                let project = stmt.query_row([&id], |row| {
                    let registered_at_str: String = row.get(3)?;
                    let registered_at = DateTime::parse_from_rfc3339(&registered_at_str)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now());

                    Ok(Project {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        path: PathBuf::from(row.get::<_, String>(2)?),
                        registered_at,
                        config_overrides: row.get(4)?,
                        metadata: row.get(5)?,
                    })
                });

                match project {
                    Ok(p) => Ok(Some(p)),
                    Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                    Err(e) => Err(tokio_rusqlite::Error::Rusqlite(e)),
                }
            })
            .await;

        result.map_err(|e| anyhow::anyhow!(e))
    }

    /// Get a project by path (canonicalized comparison)
    pub async fn get_by_path(&self, path: &Path) -> Result<Option<Project>> {
        let db = self.db.clone();
        let path_buf = path.to_path_buf();
        let canonical_query = path_buf.canonicalize().ok();

        let result = db
            .connection()
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, name, path, registered_at, config_overrides, metadata
                     FROM projects",
                )?;

                let projects = stmt.query_map([], |row| {
                    let registered_at_str: String = row.get(3)?;
                    let registered_at = DateTime::parse_from_rfc3339(&registered_at_str)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now());

                    Ok(Project {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        path: PathBuf::from(row.get::<_, String>(2)?),
                        registered_at,
                        config_overrides: row.get(4)?,
                        metadata: row.get(5)?,
                    })
                })?;

                for project_result in projects {
                    let project = project_result?;
                    // Try both exact match and canonicalized match
                    if project.path == path_buf {
                        return Ok(Some(project));
                    }
                    if let Ok(canonical_stored) = project.path.canonicalize()
                        && let Some(ref canonical_query_ref) = canonical_query
                        && canonical_stored == *canonical_query_ref
                    {
                        return Ok(Some(project));
                    }
                }
                Ok(None)
            })
            .await;

        result.map_err(|e| anyhow::anyhow!(e))
    }

    /// Remove a project by name
    ///
    /// Returns true if a project was deleted, false if no project with that name existed.
    pub async fn remove(&self, name: &str) -> Result<bool> {
        let db = self.db.clone();
        let name = name.to_string();

        let result = db
            .connection()
            .call(move |conn| {
                let tx =
                    conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

                let rows_affected = tx.execute(
                    "DELETE FROM projects WHERE name = ?",
                    rusqlite::params![&name],
                )?;

                tx.commit()?;

                Ok(rows_affected > 0)
            })
            .await;

        result.map_err(|e| anyhow::anyhow!(e))
    }
}

/// Generate a project ID with "ra-" prefix and 4 hex characters
fn generate_project_id() -> String {
    let uuid = Uuid::new_v4();
    let hex_str = uuid.to_string().replace("-", "");
    format!("ra-{}", &hex_str[..4])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_generate_project_id() {
        let id = generate_project_id();
        assert!(id.starts_with("ra-"));
        assert_eq!(id.len(), 7); // "ra-" + 4 hex chars
    }
}
