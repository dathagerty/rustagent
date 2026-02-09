use crate::db::Database;
use crate::graph::{GraphEdge, GraphNode, NodeStatus, NodeType, parent_id, validate_status};
use anyhow::{Result, anyhow};
use async_trait::async_trait;
use chrono::Utc;
use std::collections::HashMap;
use tokio_rusqlite::OptionalExtension;

/// Direction for edge queries
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeDirection {
    /// Edges going out from a node (from_node = id)
    Outgoing,
    /// Edges coming in to a node (to_node = id)
    Incoming,
    /// Both directions
    Both,
}

/// A graph containing nodes and edges (used in history/full graph queries)
#[derive(Debug, Clone)]
pub struct WorkGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

/// Query builder for flexible node searches
#[derive(Debug, Clone)]
pub struct NodeQuery {
    pub node_type: Option<NodeType>,
    pub status: Option<NodeStatus>,
    pub project_id: Option<String>,
    pub parent_id: Option<String>,
    pub query: Option<String>,
}

/// The GraphStore trait defines all operations on the work graph.
/// Implementations use BEGIN IMMEDIATE transactions for atomic writes.
#[async_trait]
pub trait GraphStore: Send + Sync {
    // ===== Node CRUD =====

    /// Create a new node in the store
    async fn create_node(&self, node: &GraphNode) -> Result<()>;

    /// Update node fields. Only provided fields are updated.
    async fn update_node(
        &self,
        id: &str,
        status: Option<NodeStatus>,
        title: Option<&str>,
        description: Option<&str>,
        metadata: Option<&HashMap<String, String>>,
    ) -> Result<()>;

    /// Retrieve a node by ID
    async fn get_node(&self, id: &str) -> Result<Option<GraphNode>>;

    /// Query nodes with optional filters
    async fn query_nodes(&self, query: &NodeQuery) -> Result<Vec<GraphNode>>;

    // ===== Task-specific operations =====

    /// Atomically claim a task (status Ready -> Claimed).
    /// Returns true if claimed, false if task was not in Ready state (another worker got it first).
    async fn claim_task(&self, node_id: &str, agent_id: &str) -> Result<bool>;

    /// Get all ready tasks under a goal
    async fn get_ready_tasks(&self, goal_id: &str) -> Result<Vec<GraphNode>>;

    /// Get the next recommended task: highest priority, break ties by downstream unblock count
    async fn get_next_task(&self, goal_id: &str) -> Result<Option<GraphNode>>;

    // ===== Edge operations =====

    /// Add an edge connecting two nodes
    async fn add_edge(&self, edge: &GraphEdge) -> Result<()>;

    /// Remove an edge by ID
    async fn remove_edge(&self, edge_id: &str) -> Result<()>;

    /// Get edges involving a node in the specified direction, with the related nodes
    async fn get_edges(
        &self,
        node_id: &str,
        direction: EdgeDirection,
    ) -> Result<Vec<(GraphEdge, GraphNode)>>;

    // ===== Graph queries =====

    /// Get immediate children of a node via Contains edges
    async fn get_children(&self, node_id: &str)
    -> Result<Vec<(GraphNode, crate::graph::EdgeType)>>;

    /// Get all descendants recursively via Contains edges
    async fn get_subtree(&self, node_id: &str) -> Result<Vec<GraphNode>>;

    /// Get active decisions for a project (decision_active or option_active, no abandoned/superseded)
    async fn get_active_decisions(&self, project_id: &str) -> Result<Vec<GraphNode>>;

    /// Get the full graph: goal + all descendants + all edges among them
    async fn get_full_graph(&self, goal_id: &str) -> Result<WorkGraph>;

    /// Full-text search nodes by query, optionally filtered by project and node_type
    async fn search_nodes(
        &self,
        query: &str,
        project_id: Option<&str>,
        node_type: Option<NodeType>,
        limit: usize,
    ) -> Result<Vec<GraphNode>>;

    // ===== Utility =====

    /// Get the next child sequence number for a parent, atomically increment, and return old value
    async fn next_child_seq(&self, parent_id: &str) -> Result<u32>;
}

/// SQLite implementation of GraphStore
pub struct SqliteGraphStore {
    db: Database,
}

impl SqliteGraphStore {
    /// Create a new SqliteGraphStore wrapping the given database
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Helper to convert a database row to a GraphNode
    fn row_to_node(row: &rusqlite::Row) -> rusqlite::Result<GraphNode> {
        let labels_json: String = row.get(10)?;
        let labels: Vec<String> = serde_json::from_str(&labels_json).unwrap_or_default();

        let metadata_json: String = row.get(14)?;
        let metadata: HashMap<String, String> =
            serde_json::from_str(&metadata_json).unwrap_or_default();

        let created_at_str: String = row.get(11)?;
        let created_at = chrono::DateTime::parse_from_rfc3339(&created_at_str)
            .ok()
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);

        let started_at_str: Option<String> = row.get(12)?;
        let started_at = started_at_str.and_then(|s| {
            chrono::DateTime::parse_from_rfc3339(&s)
                .ok()
                .map(|dt| dt.with_timezone(&Utc))
        });

        let completed_at_str: Option<String> = row.get(13)?;
        let completed_at = completed_at_str.and_then(|s| {
            chrono::DateTime::parse_from_rfc3339(&s)
                .ok()
                .map(|dt| dt.with_timezone(&Utc))
        });

        let node_type_str: String = row.get(2)?;
        let node_type = node_type_str
            .parse()
            .map_err(|_| rusqlite::Error::InvalidParameterName("Invalid node_type".to_string()))?;

        let status_str: String = row.get(5)?;
        let status = status_str
            .parse()
            .map_err(|_| rusqlite::Error::InvalidParameterName("Invalid status".to_string()))?;

        Ok(GraphNode {
            id: row.get(0)?,
            project_id: row.get(1)?,
            node_type,
            title: row.get(3)?,
            description: row.get(4)?,
            status,
            priority: row
                .get::<_, Option<String>>(6)?
                .and_then(|p| p.parse().ok()),
            assigned_to: row.get(7)?,
            created_by: row.get(8)?,
            labels,
            created_at,
            started_at,
            completed_at,
            blocked_reason: row.get(9)?,
            metadata,
        })
    }

    /// Helper to convert a database row to a GraphEdge
    fn row_to_edge(row: &rusqlite::Row) -> rusqlite::Result<GraphEdge> {
        let created_at_str: String = row.get(5)?;
        let created_at = chrono::DateTime::parse_from_rfc3339(&created_at_str)
            .ok()
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);

        let edge_type_str: String = row.get(1)?;
        let edge_type = edge_type_str
            .parse()
            .map_err(|_| rusqlite::Error::InvalidParameterName("Invalid edge_type".to_string()))?;

        Ok(GraphEdge {
            id: row.get(0)?,
            edge_type,
            from_node: row.get(2)?,
            to_node: row.get(3)?,
            label: row.get(4)?,
            created_at,
        })
    }
}

#[async_trait]
impl GraphStore for SqliteGraphStore {
    async fn create_node(&self, node: &GraphNode) -> Result<()> {
        let labels_json = serde_json::to_string(&node.labels)?;
        let metadata_json = serde_json::to_string(&node.metadata)?;
        let created_at = node.created_at.to_rfc3339();
        let started_at = node.started_at.map(|dt| dt.to_rfc3339());
        let completed_at = node.completed_at.map(|dt| dt.to_rfc3339());
        let priority = node.priority.map(|p| p.to_string());
        let node_type_str = node.node_type.to_string();
        let status_str = node.status.to_string();

        let db = self.db.clone();
        let node_id = node.id.clone();
        let project_id = node.project_id.clone();
        let title = node.title.clone();
        let description = node.description.clone();
        let assigned_to = node.assigned_to.clone();
        let created_by = node.created_by.clone();
        let blocked_reason = node.blocked_reason.clone();

        db.connection()
            .call(move |conn| {
                let tx = conn.transaction()?;

                // Insert the node
                tx.execute(
                    "INSERT INTO nodes (
                        id, project_id, node_type, title, description, status,
                        priority, assigned_to, created_by, blocked_reason,
                        labels, created_at, started_at, completed_at, metadata
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
                    rusqlite::params![
                        &node_id,
                        &project_id,
                        &node_type_str,
                        &title,
                        &description,
                        &status_str,
                        &priority,
                        &assigned_to,
                        &created_by,
                        &blocked_reason,
                        &labels_json,
                        &created_at,
                        &started_at,
                        &completed_at,
                        &metadata_json,
                    ],
                )?;

                // If this is a child node (has a parent), create a Contains edge and update parent's next_child_seq
                if let Some(p_id) = parent_id(&node_id) {
                    // Get current next_child_seq from parent metadata
                    let current_seq: Option<String> = tx.query_row(
                        "SELECT metadata FROM nodes WHERE id = ?1",
                        rusqlite::params![p_id],
                        |row| row.get(0),
                    )?;

                    let metadata: HashMap<String, String> = current_seq
                        .and_then(|s| serde_json::from_str(&s).ok())
                        .unwrap_or_default();

                    let next_seq = metadata
                        .get("next_child_seq")
                        .and_then(|s| s.parse::<u32>().ok())
                        .unwrap_or(1);

                    // Update parent's next_child_seq in metadata
                    let mut new_metadata = metadata;
                    new_metadata.insert("next_child_seq".to_string(), (next_seq + 1).to_string());
                    let new_metadata_json = serde_json::to_string(&new_metadata)
                        .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

                    tx.execute(
                        "UPDATE nodes SET metadata = ?1 WHERE id = ?2",
                        rusqlite::params![&new_metadata_json, p_id],
                    )?;

                    // Create a Contains edge from parent to child
                    let edge_id = crate::graph::generate_edge_id();
                    let now = Utc::now().to_rfc3339();
                    tx.execute(
                        "INSERT INTO edges (id, edge_type, from_node, to_node, created_at)
                         VALUES (?1, ?2, ?3, ?4, ?5)",
                        rusqlite::params![&edge_id, "contains", p_id, &node_id, &now],
                    )?;
                }

                tx.commit()?;
                Ok(())
            })
            .await?;

        Ok(())
    }

    async fn update_node(
        &self,
        id: &str,
        status: Option<NodeStatus>,
        title: Option<&str>,
        description: Option<&str>,
        metadata: Option<&HashMap<String, String>>,
    ) -> Result<()> {
        // Validate status if provided
        if let Some(s) = status {
            // Get the node to determine its type
            let node = self.get_node(id).await?;
            if let Some(n) = node {
                validate_status(&n.node_type, &s)?;
            }
        }

        let id = id.to_string();
        let status_str = status.map(|s| s.to_string());
        let title_owned = title.map(|t| t.to_string());
        let description_owned = description.map(|d| d.to_string());
        let metadata_json = metadata.map(serde_json::to_string).transpose()?;

        self.db
            .connection()
            .call(move |conn| {
                let tx = conn.transaction()?;

                // Build dynamic UPDATE statement
                let mut updates = Vec::new();
                let mut params: Vec<&dyn rusqlite::ToSql> = Vec::new();

                if let Some(s) = &status_str {
                    updates.push("status = ?");
                    params.push(s);
                }
                if let Some(t) = &title_owned {
                    updates.push("title = ?");
                    params.push(t);
                }
                if let Some(d) = &description_owned {
                    updates.push("description = ?");
                    params.push(d);
                }
                if let Some(m) = &metadata_json {
                    updates.push("metadata = ?");
                    params.push(m);
                }

                if updates.is_empty() {
                    return Ok(());
                }

                let sql = format!("UPDATE nodes SET {} WHERE id = ?", updates.join(", "));
                params.push(&id);

                tx.execute(&sql, params.as_slice())?;

                // If status changed to Completed, check for dependent tasks to move to Ready
                if let Some(NodeStatus::Completed) = status {
                    // Find all nodes that DependsOn this node
                    let mut stmt = tx.prepare(
                        "SELECT from_node FROM edges WHERE to_node = ?1 AND edge_type = 'depends_on'"
                    )?;

                    let dependent_ids: Vec<String> = stmt
                        .query_map(rusqlite::params![&id], |row| row.get(0))?
                        .collect::<Result<Vec<_>, _>>()?;

                    for dependent_id in dependent_ids {
                        // Check if all dependencies are now completed
                        let all_deps_completed: bool = tx
                            .query_row(
                                "SELECT COUNT(*) = 0 FROM edges
                                 WHERE from_node = ?1 AND edge_type = 'depends_on'
                                 AND to_node NOT IN (SELECT id FROM nodes WHERE status = 'completed')",
                                rusqlite::params![&dependent_id],
                                |row| row.get(0),
                            )?;

                        if all_deps_completed {
                            // Move this task to Ready if it's currently Pending
                            tx.execute(
                                "UPDATE nodes SET status = 'ready'
                                 WHERE id = ?1 AND status = 'pending'",
                                rusqlite::params![&dependent_id],
                            )?;
                        }
                    }
                }

                tx.commit()?;
                Ok(())
            })
            .await?;

        Ok(())
    }

    async fn get_node(&self, id: &str) -> Result<Option<GraphNode>> {
        let id = id.to_string();

        let result = self
            .db
            .connection()
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, project_id, node_type, title, description, status,
                            priority, assigned_to, created_by, blocked_reason,
                            labels, created_at, started_at, completed_at, metadata
                     FROM nodes WHERE id = ?1",
                )?;

                let node = stmt
                    .query_row(rusqlite::params![&id], Self::row_to_node)
                    .optional()?;

                Ok(node)
            })
            .await?;

        Ok(result)
    }

    async fn query_nodes(&self, query: &NodeQuery) -> Result<Vec<GraphNode>> {
        let node_type = query.node_type;
        let status = query.status;
        let project_id = query.project_id.clone();
        let parent_id_filter = query.parent_id.clone();

        let result = self
            .db
            .connection()
            .call(move |conn| {
                let mut sql = "SELECT id, project_id, node_type, title, description, status,
                                     priority, assigned_to, created_by, blocked_reason,
                                     labels, created_at, started_at, completed_at, metadata
                              FROM nodes WHERE 1=1".to_string();

                let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

                if let Some(nt) = node_type {
                    sql.push_str(" AND node_type = ?");
                    params.push(Box::new(nt.to_string()));
                }
                if let Some(s) = status {
                    sql.push_str(" AND status = ?");
                    params.push(Box::new(s.to_string()));
                }
                if let Some(pid) = &project_id {
                    sql.push_str(" AND project_id = ?");
                    params.push(Box::new(pid.clone()));
                }
                if let Some(parent) = &parent_id_filter {
                    // Find nodes whose parent is the given parent_id
                    sql.push_str(" AND id IN (SELECT to_node FROM edges WHERE from_node = ? AND edge_type = 'contains')");
                    params.push(Box::new(parent.clone()));
                }

                let mut stmt = conn.prepare(&sql)?;
                let rows = stmt
                    .query_map(
                        rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
                        Self::row_to_node,
                    )?
                    .collect::<Result<Vec<_>, _>>()?;

                Ok(rows)
            })
            .await?;

        Ok(result)
    }

    async fn claim_task(&self, node_id: &str, agent_id: &str) -> Result<bool> {
        let node_id = node_id.to_string();
        let agent_id = agent_id.to_string();
        let now = Utc::now().to_rfc3339();

        let claimed = self
            .db
            .connection()
            .call(move |conn| {
                let tx = conn.transaction()?;

                tx.execute(
                    "UPDATE nodes SET status = 'claimed', assigned_to = ?1, started_at = ?2
                     WHERE id = ?3 AND status = 'ready'",
                    rusqlite::params![&agent_id, &now, &node_id],
                )?;

                let success = tx.changes() > 0;
                tx.commit()?;
                Ok(success)
            })
            .await?;

        Ok(claimed)
    }

    async fn get_ready_tasks(&self, goal_id: &str) -> Result<Vec<GraphNode>> {
        let goal_id = goal_id.to_string();

        let result = self
            .db
            .connection()
            .call(move |conn| {
                // Get all nodes under the goal that are Ready
                let mut stmt = conn.prepare(
                    "WITH RECURSIVE subtree AS (
                        SELECT id FROM nodes WHERE id = ?1
                        UNION ALL
                        SELECT e.to_node FROM edges e
                        JOIN subtree s ON e.from_node = s.id
                        WHERE e.edge_type = 'contains'
                    )
                    SELECT id, project_id, node_type, title, description, status,
                           priority, assigned_to, created_by, blocked_reason,
                           labels, created_at, started_at, completed_at, metadata
                    FROM nodes
                    WHERE id IN (SELECT id FROM subtree)
                      AND status = 'ready'
                    ORDER BY created_at",
                )?;

                let nodes = stmt
                    .query_map(rusqlite::params![&goal_id], Self::row_to_node)?
                    .collect::<Result<Vec<_>, _>>()?;

                Ok(nodes)
            })
            .await?;

        Ok(result)
    }

    async fn get_next_task(&self, goal_id: &str) -> Result<Option<GraphNode>> {
        let goal_id = goal_id.to_string();

        let result = self
            .db
            .connection()
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "WITH RECURSIVE subtree AS (
                        SELECT id FROM nodes WHERE id = ?1
                        UNION ALL
                        SELECT e.to_node FROM edges e
                        JOIN subtree s ON e.from_node = s.id
                        WHERE e.edge_type = 'contains'
                    ),
                    ready_tasks AS (
                        SELECT id, priority FROM nodes
                        WHERE id IN (SELECT id FROM subtree)
                          AND status = 'ready'
                    ),
                    downstream_counts AS (
                        SELECT rt.id, COUNT(*) as downstream_count
                        FROM ready_tasks rt
                        LEFT JOIN (
                            WITH RECURSIVE transitive_deps AS (
                                SELECT from_node as start_node, to_node FROM edges
                                WHERE edge_type = 'depends_on'
                                UNION ALL
                                SELECT td.start_node, e.to_node FROM transitive_deps td
                                JOIN edges e ON e.from_node = td.to_node
                                WHERE e.edge_type = 'depends_on'
                            )
                            SELECT DISTINCT start_node FROM transitive_deps
                        ) td ON rt.id = td.start_node
                        GROUP BY rt.id
                    )
                    SELECT n.id, n.project_id, n.node_type, n.title, n.description, n.status,
                           n.priority, n.assigned_to, n.created_by, n.blocked_reason,
                           n.labels, n.created_at, n.started_at, n.completed_at, n.metadata
                    FROM nodes n
                    JOIN downstream_counts dc ON n.id = dc.id
                    ORDER BY
                        CASE WHEN n.priority = 'critical' THEN 0
                             WHEN n.priority = 'high' THEN 1
                             WHEN n.priority = 'medium' THEN 2
                             WHEN n.priority = 'low' THEN 3
                             ELSE 4 END,
                        dc.downstream_count DESC,
                        n.created_at ASC
                    LIMIT 1",
                )?;

                let node = stmt
                    .query_row(rusqlite::params![&goal_id], Self::row_to_node)
                    .optional()?;

                Ok(node)
            })
            .await?;

        Ok(result)
    }

    async fn add_edge(&self, edge: &GraphEdge) -> Result<()> {
        // Validate that both nodes exist
        let from_node = self.get_node(&edge.from_node).await?;
        let to_node = self.get_node(&edge.to_node).await?;

        if from_node.is_none() {
            return Err(anyhow!("from_node does not exist: {}", edge.from_node));
        }
        if to_node.is_none() {
            return Err(anyhow!("to_node does not exist: {}", edge.to_node));
        }

        let edge_id = edge.id.clone();
        let edge_type = edge.edge_type.to_string();
        let from_node_id = edge.from_node.clone();
        let to_node_id = edge.to_node.clone();
        let label = edge.label.clone();
        let created_at = edge.created_at.to_rfc3339();

        self.db
            .connection()
            .call(move |conn| {
                conn.execute(
                    "INSERT INTO edges (id, edge_type, from_node, to_node, label, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    rusqlite::params![
                        &edge_id,
                        &edge_type,
                        &from_node_id,
                        &to_node_id,
                        &label,
                        &created_at
                    ],
                )?;
                Ok(())
            })
            .await?;

        Ok(())
    }

    async fn remove_edge(&self, edge_id: &str) -> Result<()> {
        let edge_id = edge_id.to_string();

        self.db
            .connection()
            .call(move |conn| {
                conn.execute(
                    "DELETE FROM edges WHERE id = ?1",
                    rusqlite::params![&edge_id],
                )?;
                Ok(())
            })
            .await?;

        Ok(())
    }

    async fn get_edges(
        &self,
        node_id: &str,
        direction: EdgeDirection,
    ) -> Result<Vec<(GraphEdge, GraphNode)>> {
        let node_id = node_id.to_string();

        let result = self
            .db
            .connection()
            .call(move |conn| {
                // Get edges first
                let mut edge_stmt = match direction {
                    EdgeDirection::Outgoing => conn.prepare(
                        "SELECT id, edge_type, from_node, to_node, label, created_at
                         FROM edges WHERE from_node = ?1",
                    )?,
                    EdgeDirection::Incoming => conn.prepare(
                        "SELECT id, edge_type, from_node, to_node, label, created_at
                         FROM edges WHERE to_node = ?1",
                    )?,
                    EdgeDirection::Both => conn.prepare(
                        "SELECT id, edge_type, from_node, to_node, label, created_at
                         FROM edges WHERE from_node = ?1 OR to_node = ?1",
                    )?,
                };

                let edges: Vec<GraphEdge> = edge_stmt
                    .query_map(rusqlite::params![&node_id], Self::row_to_edge)?
                    .collect::<Result<Vec<_>, _>>()?;

                // For each edge, get the related node
                let mut pairs = Vec::new();
                for edge in edges {
                    let related_id = match direction {
                        EdgeDirection::Outgoing => &edge.to_node,
                        EdgeDirection::Incoming => &edge.from_node,
                        EdgeDirection::Both => {
                            if edge.from_node == node_id {
                                &edge.to_node
                            } else {
                                &edge.from_node
                            }
                        }
                    };

                    let node = conn.query_row(
                        "SELECT id, project_id, node_type, title, description, status,
                                    priority, assigned_to, created_by, blocked_reason,
                                    labels, created_at, started_at, completed_at, metadata
                             FROM nodes WHERE id = ?1",
                        rusqlite::params![related_id],
                        Self::row_to_node,
                    )?;

                    pairs.push((edge, node));
                }

                Ok(pairs)
            })
            .await?;

        Ok(result)
    }

    async fn get_children(
        &self,
        node_id: &str,
    ) -> Result<Vec<(GraphNode, crate::graph::EdgeType)>> {
        let node_id = node_id.to_string();

        let result = self
            .db
            .connection()
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT n.id, n.project_id, n.node_type, n.title, n.description, n.status,
                            n.priority, n.assigned_to, n.created_by, n.blocked_reason,
                            n.labels, n.created_at, n.started_at, n.completed_at, n.metadata,
                            e.edge_type
                     FROM nodes n
                     JOIN edges e ON e.to_node = n.id
                     WHERE e.from_node = ?1 AND e.edge_type = 'contains'
                     ORDER BY n.created_at",
                )?;

                let children = stmt
                    .query_map(rusqlite::params![&node_id], |row| {
                        let node = Self::row_to_node(row)?;
                        let edge_type_str: String = row.get(15)?;
                        let edge_type = edge_type_str.parse().map_err(|_| {
                            rusqlite::Error::InvalidParameterName("Invalid edge_type".to_string())
                        })?;
                        Ok((node, edge_type))
                    })?
                    .collect::<Result<Vec<_>, _>>()?;

                Ok(children)
            })
            .await?;

        Ok(result)
    }

    async fn get_subtree(&self, node_id: &str) -> Result<Vec<GraphNode>> {
        let node_id = node_id.to_string();

        let result = self
            .db
            .connection()
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "WITH RECURSIVE subtree AS (
                        SELECT id FROM nodes WHERE id = ?1
                        UNION ALL
                        SELECT e.to_node FROM edges e
                        JOIN subtree s ON e.from_node = s.id
                        WHERE e.edge_type = 'contains'
                    )
                    SELECT id, project_id, node_type, title, description, status,
                           priority, assigned_to, created_by, blocked_reason,
                           labels, created_at, started_at, completed_at, metadata
                    FROM nodes
                    WHERE id IN (SELECT id FROM subtree)
                    ORDER BY created_at",
                )?;

                let nodes = stmt
                    .query_map(rusqlite::params![&node_id], Self::row_to_node)?
                    .collect::<Result<Vec<_>, _>>()?;

                Ok(nodes)
            })
            .await?;

        Ok(result)
    }

    async fn get_active_decisions(&self, project_id: &str) -> Result<Vec<GraphNode>> {
        let project_id = project_id.to_string();

        let result = self
            .db
            .connection()
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, project_id, node_type, title, description, status,
                            priority, assigned_to, created_by, blocked_reason,
                            labels, created_at, started_at, completed_at, metadata
                     FROM nodes
                     WHERE project_id = ?1
                       AND node_type = 'decision'
                       AND (status = 'active' OR status = 'decided')
                     ORDER BY created_at",
                )?;

                let nodes = stmt
                    .query_map(rusqlite::params![&project_id], Self::row_to_node)?
                    .collect::<Result<Vec<_>, _>>()?;

                Ok(nodes)
            })
            .await?;

        Ok(result)
    }

    async fn get_full_graph(&self, goal_id: &str) -> Result<WorkGraph> {
        let goal_id = goal_id.to_string();

        let result = self
            .db
            .connection()
            .call(move |conn| {
                // Get all nodes in the subtree
                let mut stmt = conn.prepare(
                    "WITH RECURSIVE node_tree AS (
                        SELECT id FROM nodes WHERE id = ?1
                        UNION ALL
                        SELECT e.to_node FROM edges e
                        JOIN node_tree nt ON e.from_node = nt.id
                        WHERE e.edge_type = 'contains'
                    )
                    SELECT id, project_id, node_type, title, description, status,
                           priority, assigned_to, created_by, blocked_reason,
                           labels, created_at, started_at, completed_at, metadata
                    FROM nodes
                    WHERE id IN (SELECT id FROM node_tree)
                    ORDER BY created_at",
                )?;

                let nodes = stmt
                    .query_map(rusqlite::params![&goal_id], Self::row_to_node)?
                    .collect::<Result<Vec<_>, _>>()?;

                // Get all edges between nodes in the subtree
                let mut stmt = conn.prepare(
                    "WITH RECURSIVE node_tree AS (
                        SELECT id FROM nodes WHERE id = ?1
                        UNION ALL
                        SELECT e.to_node FROM edges e
                        JOIN node_tree nt ON e.from_node = nt.id
                        WHERE e.edge_type = 'contains'
                    )
                    SELECT e.id, e.edge_type, e.from_node, e.to_node, e.label, e.created_at
                    FROM edges e
                    WHERE e.from_node IN (SELECT id FROM node_tree)
                       OR e.to_node IN (SELECT id FROM node_tree)",
                )?;

                let edges = stmt
                    .query_map(rusqlite::params![&goal_id], Self::row_to_edge)?
                    .collect::<Result<Vec<_>, _>>()?;

                Ok(WorkGraph { nodes, edges })
            })
            .await?;

        Ok(result)
    }

    async fn search_nodes(
        &self,
        query: &str,
        project_id: Option<&str>,
        node_type: Option<NodeType>,
        limit: usize,
    ) -> Result<Vec<GraphNode>> {
        let query_str = query.to_string();
        let project_id_filter = project_id.map(|p| p.to_string());
        let node_type_filter = node_type.map(|nt| nt.to_string());
        let limit_val = std::cmp::max(limit, 1);

        let result = self
            .db
            .connection()
            .call(move |conn| {
                let mut sql = "SELECT n.id, n.project_id, n.node_type, n.title, n.description, n.status,
                                      n.priority, n.assigned_to, n.created_by, n.blocked_reason,
                                      n.labels, n.created_at, n.started_at, n.completed_at, n.metadata
                               FROM nodes n
                               JOIN nodes_fts fts ON n.rowid = fts.rowid
                               WHERE nodes_fts MATCH ?1"
                    .to_string();

                let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(query_str)];

                if let Some(pid) = &project_id_filter {
                    sql.push_str(" AND n.project_id = ?");
                    params.push(Box::new(pid.clone()));
                }
                if let Some(nt) = &node_type_filter {
                    sql.push_str(" AND n.node_type = ?");
                    params.push(Box::new(nt.clone()));
                }

                sql.push_str(&format!(" LIMIT {}", limit_val));

                let mut stmt = conn.prepare(&sql)?;
                let nodes = stmt
                    .query_map(
                        rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
                        Self::row_to_node,
                    )?
                    .collect::<Result<Vec<_>, _>>()?;

                Ok(nodes)
            })
            .await?;

        Ok(result)
    }

    async fn next_child_seq(&self, parent_id: &str) -> Result<u32> {
        let parent_id = parent_id.to_string();

        let seq = self
            .db
            .connection()
            .call(move |conn| {
                let tx = conn.transaction()?;

                // Get current metadata
                let metadata_json: String = tx.query_row(
                    "SELECT metadata FROM nodes WHERE id = ?1",
                    rusqlite::params![&parent_id],
                    |row| row.get(0),
                )?;

                let mut metadata: HashMap<String, String> =
                    serde_json::from_str(&metadata_json).unwrap_or_default();

                let current_seq = metadata
                    .get("next_child_seq")
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(1);

                // Update to next value
                metadata.insert("next_child_seq".to_string(), (current_seq + 1).to_string());
                let new_metadata_json = serde_json::to_string(&metadata)
                    .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

                tx.execute(
                    "UPDATE nodes SET metadata = ?1 WHERE id = ?2",
                    rusqlite::params![&new_metadata_json, &parent_id],
                )?;

                tx.commit()?;
                Ok(current_seq)
            })
            .await?;

        Ok(seq)
    }
}
