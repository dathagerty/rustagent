/// Check if all dependencies for a node are satisfied (completed)
///
/// Queries all DependsOn edges FROM the given node_id and checks if all
/// target nodes have status 'completed'. Returns true if all dependencies are met,
/// or if the node has no dependencies.
pub fn check_dependencies_met(
    conn: &rusqlite::Connection,
    node_id: &str,
) -> rusqlite::Result<bool> {
    // Count how many DependsOn edges FROM this node point to non-completed nodes
    let unmet_deps_count: u32 = conn.query_row(
        "SELECT COUNT(*) FROM edges e
         JOIN nodes n ON e.to_node = n.id
         WHERE e.from_node = ?1 AND e.edge_type = 'depends_on' AND n.status != 'completed'",
        rusqlite::params![node_id],
        |row| row.get(0),
    )?;

    Ok(unmet_deps_count == 0)
}
