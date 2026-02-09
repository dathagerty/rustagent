use crate::db::Database;
use crate::graph::store::SqliteGraphStore;
use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio_rusqlite::OptionalExtension;
use uuid::Uuid;

/// A session represents a work period for a goal
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub project_id: String,
    pub goal_id: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub handoff_notes: Option<String>,
    pub agent_ids: Vec<String>,
    pub summary: Option<String>,
}

/// Store for managing sessions
pub struct SessionStore {
    db: Database,
}

impl SessionStore {
    /// Create a new SessionStore
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Create a new session for a goal
    pub async fn create_session(&self, project_id: &str, goal_id: &str) -> Result<Session> {
        let session_id = format!("sess-{}", &Uuid::new_v4().simple().to_string()[..8]);
        let now = Utc::now();
        let now_rfc3339 = now.to_rfc3339();
        let project_id_owned = project_id.to_string();
        let goal_id_owned = goal_id.to_string();
        let session_id_clone = session_id.clone();

        self.db
            .connection()
            .call(move |conn| {
                conn.execute(
                    "INSERT INTO sessions (id, project_id, goal_id, started_at, agent_ids)
                     VALUES (?, ?, ?, ?, ?)",
                    rusqlite::params![
                        &session_id_clone,
                        &project_id_owned,
                        &goal_id_owned,
                        &now_rfc3339,
                        "[]"
                    ],
                )
                .map_err(tokio_rusqlite::Error::Rusqlite)
            })
            .await
            .map_err(|e| anyhow!("failed to insert session: {}", e))?;

        Ok(Session {
            id: session_id,
            project_id: project_id.to_string(),
            goal_id: goal_id.to_string(),
            started_at: now,
            ended_at: None,
            handoff_notes: None,
            agent_ids: vec![],
            summary: None,
        })
    }

    /// End a session and generate handoff notes
    pub async fn end_session(
        &self,
        session_id: &str,
        _graph_store: &SqliteGraphStore,
    ) -> Result<()> {
        // First, get the session to find the goal_id
        let session = self
            .get_session(session_id)
            .await?
            .ok_or_else(|| anyhow!("session not found: {}", session_id))?;

        let goal_id = session.goal_id.clone();
        let now = Utc::now();
        let now_rfc3339 = now.to_rfc3339();
        let session_id_owned = session_id.to_string();

        // Generate handoff notes within a transaction
        self.db
            .connection()
            .call(move |conn| {
                conn.execute_batch("BEGIN IMMEDIATE")
                    .map_err(tokio_rusqlite::Error::Rusqlite)?;
                let notes = generate_handoff_notes(conn, &goal_id)
                    .map_err(tokio_rusqlite::Error::Rusqlite)?;
                conn.execute(
                    "UPDATE sessions SET ended_at = ?, handoff_notes = ? WHERE id = ?",
                    rusqlite::params![&now_rfc3339, &notes, &session_id_owned],
                )
                .map_err(tokio_rusqlite::Error::Rusqlite)?;
                conn.execute_batch("COMMIT")
                    .map_err(tokio_rusqlite::Error::Rusqlite)?;
                Ok::<(), tokio_rusqlite::Error>(())
            })
            .await
            .map_err(|e| anyhow!("database error: {}", e))?;

        Ok(())
    }

    /// Get a session by ID
    pub async fn get_session(&self, session_id: &str) -> Result<Option<Session>> {
        let session_id_owned = session_id.to_string();

        let result = self
            .db
            .connection()
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, project_id, goal_id, started_at, ended_at, handoff_notes, agent_ids, summary
                     FROM sessions WHERE id = ?",
                )?;

                let session: Option<Session> = stmt
                    .query_row([&session_id_owned], |row| {
                        let started_at_str: String = row.get(3)?;
                        let started_at = chrono::DateTime::parse_from_rfc3339(&started_at_str)
                            .ok()
                            .map(|dt| dt.with_timezone(&Utc))
                            .ok_or(rusqlite::Error::InvalidQuery)?;

                        let ended_at_str: Option<String> = row.get(4)?;
                        let ended_at = ended_at_str.and_then(|s| {
                            chrono::DateTime::parse_from_rfc3339(&s)
                                .ok()
                                .map(|dt| dt.with_timezone(&Utc))
                        });

                        let agent_ids_json: String = row.get(6)?;
                        let agent_ids: Vec<String> =
                            serde_json::from_str(&agent_ids_json).unwrap_or_default();

                        Ok(Session {
                            id: row.get(0)?,
                            project_id: row.get(1)?,
                            goal_id: row.get(2)?,
                            started_at,
                            ended_at,
                            handoff_notes: row.get(5)?,
                            agent_ids,
                            summary: row.get(7)?,
                        })
                    })
                    .optional()?;

                Ok(session)
            })
            .await
            .map_err(|e| anyhow!("database error: {}", e))?;

        Ok(result)
    }

    /// Get the most recent session for a goal
    pub async fn get_latest_session(&self, goal_id: &str) -> Result<Option<Session>> {
        let goal_id_owned = goal_id.to_string();

        let result = self
            .db
            .connection()
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, project_id, goal_id, started_at, ended_at, handoff_notes, agent_ids, summary
                     FROM sessions WHERE goal_id = ?
                     ORDER BY started_at DESC LIMIT 1",
                )?;

                let session: Option<Session> = stmt
                    .query_row([&goal_id_owned], |row| {
                        let started_at_str: String = row.get(3)?;
                        let started_at = chrono::DateTime::parse_from_rfc3339(&started_at_str)
                            .ok()
                            .map(|dt| dt.with_timezone(&Utc))
                            .ok_or(rusqlite::Error::InvalidQuery)?;

                        let ended_at_str: Option<String> = row.get(4)?;
                        let ended_at = ended_at_str.and_then(|s| {
                            chrono::DateTime::parse_from_rfc3339(&s)
                                .ok()
                                .map(|dt| dt.with_timezone(&Utc))
                        });

                        let agent_ids_json: String = row.get(6)?;
                        let agent_ids: Vec<String> =
                            serde_json::from_str(&agent_ids_json).unwrap_or_default();

                        Ok(Session {
                            id: row.get(0)?,
                            project_id: row.get(1)?,
                            goal_id: row.get(2)?,
                            started_at,
                            ended_at,
                            handoff_notes: row.get(5)?,
                            agent_ids,
                            summary: row.get(7)?,
                        })
                    })
                    .optional()?;

                Ok(session)
            })
            .await
            .map_err(|e| anyhow!("database error: {}", e))?;

        Ok(result)
    }

    /// List all sessions for a goal
    pub async fn list_sessions(&self, goal_id: &str) -> Result<Vec<Session>> {
        let goal_id_owned = goal_id.to_string();

        self.db
            .connection()
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, project_id, goal_id, started_at, ended_at, handoff_notes, agent_ids, summary
                     FROM sessions WHERE goal_id = ?
                     ORDER BY started_at DESC",
                )?;

                let mut sessions = vec![];
                let rows = stmt.query_map([&goal_id_owned], |row| {
                    let started_at_str: String = row.get(3)?;
                    let started_at = chrono::DateTime::parse_from_rfc3339(&started_at_str)
                        .ok()
                        .map(|dt| dt.with_timezone(&Utc))
                        .ok_or(rusqlite::Error::InvalidQuery)?;

                    let ended_at_str: Option<String> = row.get(4)?;
                    let ended_at = ended_at_str.and_then(|s| {
                        chrono::DateTime::parse_from_rfc3339(&s)
                            .ok()
                            .map(|dt| dt.with_timezone(&Utc))
                    });

                    let agent_ids_json: String = row.get(6)?;
                    let agent_ids: Vec<String> =
                        serde_json::from_str(&agent_ids_json).unwrap_or_default();

                    Ok(Session {
                        id: row.get(0)?,
                        project_id: row.get(1)?,
                        goal_id: row.get(2)?,
                        started_at,
                        ended_at,
                        handoff_notes: row.get(5)?,
                        agent_ids,
                        summary: row.get(7)?,
                    })
                })?;

                for session_result in rows {
                    sessions.push(session_result?);
                }

                Ok(sessions)
            })
            .await
            .map_err(|e| anyhow!("database error: {}", e))
    }
}

/// Generate handoff notes from the current graph state
/// This runs synchronously within a transaction on the raw rusqlite connection
fn generate_handoff_notes(conn: &rusqlite::Connection, goal_id: &str) -> rusqlite::Result<String> {
    let mut notes = String::new();

    // Query all descendants of the goal
    let descendants = get_descendants(conn, goal_id)?;
    let descendant_ids: Vec<String> = descendants.iter().map(|d| d.0.clone()).collect();

    if descendant_ids.is_empty() {
        // No descendants, return empty template
        notes.push_str("## Done\n\n");
        notes.push_str("## Remaining\n\n");
        notes.push_str("## Blocked\n\n");
        notes.push_str("## Decisions Made\n\n");
        return Ok(notes);
    }

    // Build placeholders for SQL IN clause
    let placeholders = descendant_ids
        .iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(",");

    // Query for Done nodes (Completed or Decided)
    let done_query = format!(
        "SELECT id, title, status FROM nodes WHERE id IN ({})
         AND (status = 'completed' OR status = 'decided')
         ORDER BY completed_at ASC, created_at ASC",
        placeholders
    );
    notes.push_str("## Done\n");
    let mut stmt = conn.prepare(&done_query)?;
    let done_nodes: Vec<_> = stmt
        .query_map(
            rusqlite::params_from_iter(descendant_ids.iter().map(|s| s.as_str())),
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;
    if done_nodes.is_empty() {
        notes.push_str("(none)\n");
    } else {
        for (id, title, status) in done_nodes {
            notes.push_str(&format!("- {}: {} ({})\n", id, title, status));
        }
    }
    notes.push('\n');

    // Query for Remaining nodes (Ready, Pending, InProgress)
    let remaining_query = format!(
        "SELECT id, title, status FROM nodes WHERE id IN ({})
         AND (status = 'ready' OR status = 'pending' OR status = 'in_progress')
         ORDER BY created_at ASC",
        placeholders
    );
    notes.push_str("## Remaining\n");
    let mut stmt = conn.prepare(&remaining_query)?;
    let remaining_nodes: Vec<_> = stmt
        .query_map(
            rusqlite::params_from_iter(descendant_ids.iter().map(|s| s.as_str())),
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;
    if remaining_nodes.is_empty() {
        notes.push_str("(none)\n");
    } else {
        for (id, title, _status) in remaining_nodes {
            notes.push_str(&format!("- {}: {}\n", id, title));
        }
    }
    notes.push('\n');

    // Query for Blocked nodes
    let blocked_query = format!(
        "SELECT id, title, blocked_reason FROM nodes WHERE id IN ({})
         AND status = 'blocked'
         ORDER BY created_at ASC",
        placeholders
    );
    notes.push_str("## Blocked\n");
    let mut stmt = conn.prepare(&blocked_query)?;
    let blocked_nodes: Vec<_> = stmt
        .query_map(
            rusqlite::params_from_iter(descendant_ids.iter().map(|s| s.as_str())),
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;
    if blocked_nodes.is_empty() {
        notes.push_str("(none)\n");
    } else {
        for (id, title, blocked_reason) in blocked_nodes {
            if let Some(reason) = blocked_reason {
                notes.push_str(&format!("- {}: {} — {}\n", id, title, reason));
            } else {
                notes.push_str(&format!("- {}: {}\n", id, title));
            }
        }
    }
    notes.push('\n');

    // Query for Decisions Made (Decision nodes with Decided status)
    let decisions_query = format!(
        "SELECT id, title FROM nodes WHERE id IN ({})
         AND node_type = 'decision' AND status = 'decided'
         ORDER BY created_at ASC",
        placeholders
    );
    notes.push_str("## Decisions Made\n");
    let mut stmt = conn.prepare(&decisions_query)?;
    let decision_nodes: Vec<_> = stmt
        .query_map(
            rusqlite::params_from_iter(descendant_ids.iter().map(|s| s.as_str())),
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )?
        .collect::<Result<Vec<_>, _>>()?;
    if decision_nodes.is_empty() {
        notes.push_str("(none)\n");
    } else {
        for (decision_id, decision_title) in decision_nodes {
            // Find the chosen option
            let mut chosen_stmt = conn.prepare(
                "SELECT n.title, e.label FROM edges e
                 JOIN nodes n ON e.to_node = n.id
                 WHERE e.from_node = ? AND e.edge_type = 'chosen'",
            )?;

            let chosen_option: Option<(String, Option<String>)> = chosen_stmt
                .query_row([&decision_id], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
                })
                .optional()?;

            if let Some((option_title, rationale)) = chosen_option {
                if let Some(r) = rationale {
                    notes.push_str(&format!(
                        "- {}: {} → {} ({})\n",
                        decision_id, decision_title, option_title, r
                    ));
                } else {
                    notes.push_str(&format!(
                        "- {}: {} → {}\n",
                        decision_id, decision_title, option_title
                    ));
                }
            } else {
                notes.push_str(&format!("- {}: {}\n", decision_id, decision_title));
            }
        }
    }
    notes.push('\n');

    Ok(notes)
}

/// Get all descendants of a node via Contains edges (helper for handoff notes)
fn get_descendants(
    conn: &rusqlite::Connection,
    parent_id: &str,
) -> rusqlite::Result<Vec<(String, String)>> {
    // Recursive CTE to get all descendants
    let query = "
        WITH RECURSIVE descendants AS (
            SELECT id, node_type FROM nodes WHERE id = ?
            UNION ALL
            SELECT n.id, n.node_type FROM nodes n
            JOIN edges e ON n.id = e.to_node
            JOIN descendants d ON e.from_node = d.id
            WHERE e.edge_type = 'contains'
        )
        SELECT id, node_type FROM descendants WHERE id != ?
    ";

    let mut stmt = conn.prepare(query)?;
    let descendants = stmt
        .query_map(rusqlite::params![parent_id, parent_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(descendants)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_session_creation() {
        // This test structure will be used in session_test.rs
        // Just verify Session struct can be created
        let session = Session {
            id: "sess-test".to_string(),
            project_id: "proj-1".to_string(),
            goal_id: "ra-1234".to_string(),
            started_at: Utc::now(),
            ended_at: None,
            handoff_notes: None,
            agent_ids: vec![],
            summary: None,
        };
        assert_eq!(session.id, "sess-test");
    }
}
