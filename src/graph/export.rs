use crate::graph::store::{GraphStore, NodeQuery, SqliteGraphStore};
use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

/// Export all decisions from a project as ADR markdown files
pub async fn export_adrs(
    graph_store: &SqliteGraphStore,
    project_id: &str,
    output_dir: &Path,
) -> Result<Vec<PathBuf>> {
    // Create output directory if it doesn't exist
    fs::create_dir_all(output_dir)?;

    // Query all decision nodes for the project
    let decisions = graph_store
        .query_nodes(&NodeQuery {
            node_type: Some(crate::graph::NodeType::Decision),
            status: None,
            project_id: Some(project_id.to_string()),
            parent_id: None,
            query: None,
        })
        .await?;

    // Sort by created_at for sequential numbering
    let mut decisions = decisions;
    decisions.sort_by_key(|d| d.created_at);

    let mut output_files = vec![];

    for (index, decision) in decisions.iter().enumerate() {
        let number = index + 1;
        let number_str = format!("{:03}", number);
        let slug = slugify(&decision.title);
        let filename = format!("{}-{}.md", number_str, slug);
        let file_path = output_dir.join(&filename);

        // Generate markdown content
        let content = generate_adr_markdown(graph_store, decision, &number_str).await?;

        // Write file
        fs::write(&file_path, &content)?;
        output_files.push(file_path);
    }

    Ok(output_files)
}

/// Generate ADR markdown for a decision node
async fn generate_adr_markdown(
    graph_store: &SqliteGraphStore,
    decision: &crate::graph::GraphNode,
    number: &str,
) -> Result<String> {
    let mut content = String::new();

    // Title
    content.push_str(&format!("# ADR {}: {}\n\n", number, decision.title));

    // Status
    content.push_str(&format!("**Status:** {}\n\n", decision.status));

    // Context
    content.push_str(&format!("## Context\n\n{}\n\n", decision.description));

    // Options Considered
    content.push_str("## Options Considered\n\n");

    // Get options connected via LeadsTo edges
    let options = graph_store
        .get_edges(&decision.id, crate::graph::store::EdgeDirection::Outgoing)
        .await?;

    let mut has_options = false;
    for (edge, option_node) in &options {
        if edge.edge_type == crate::graph::EdgeType::LeadsTo {
            has_options = true;
            // Check if this option was chosen or rejected via Chosen/Rejected edges
            let status_edges = graph_store
                .get_edges(&decision.id, crate::graph::store::EdgeDirection::Outgoing)
                .await?;

            let mut is_chosen = false;
            let mut rationale = String::new();

            for (status_edge, _node) in &status_edges {
                if status_edge.edge_type == crate::graph::EdgeType::Chosen
                    && status_edge.to_node == option_node.id
                {
                    is_chosen = true;
                    if let Some(label) = &status_edge.label {
                        rationale = label.clone();
                    }
                }
            }

            let status_label = if is_chosen { "CHOSEN" } else { "REJECTED" };

            content.push_str(&format!("### {} ({})\n\n", option_node.title, status_label));

            if !option_node.description.is_empty() {
                content.push_str(&format!("{}\n\n", option_node.description));
            }

            if !rationale.is_empty() {
                content.push_str(&format!("**Rationale:** {}\n\n", rationale));
            }

            // Add pros/cons from metadata if available
            if let Some(pros) = option_node.metadata.get("pros") {
                content.push_str(&format!("**Pros:**\n{}\n\n", pros));
            }
            if let Some(cons) = option_node.metadata.get("cons") {
                content.push_str(&format!("**Cons:**\n{}\n\n", cons));
            }
        }
    }

    if !has_options {
        content.push_str("(None documented)\n\n");
    }

    // Outcome
    content.push_str("## Outcome\n\n");
    if let Some(outcome) = &decision.metadata.get("outcome") {
        content.push_str(outcome);
    } else {
        content.push_str("(Pending)");
    }
    content.push_str("\n\n");

    // Related Tasks
    content.push_str("## Related Tasks\n\n");
    let related_tasks = graph_store
        .get_edges(&decision.id, crate::graph::store::EdgeDirection::Both)
        .await?;

    let mut task_lines = vec![];
    for (edge, node) in &related_tasks {
        if node.node_type == crate::graph::NodeType::Task {
            match edge.edge_type {
                crate::graph::EdgeType::LeadsTo => {
                    task_lines.push(format!("- {} (leads to task)", node.id));
                }
                crate::graph::EdgeType::DependsOn => {
                    task_lines.push(format!("- {} (depends on task)", node.id));
                }
                crate::graph::EdgeType::Informs => {
                    task_lines.push(format!("- {} (informs task)", node.id));
                }
                _ => {}
            }
        }
    }

    if task_lines.is_empty() {
        content.push_str("(None)\n\n");
    } else {
        for line in task_lines {
            content.push_str(&format!("{}\n", line));
        }
        content.push('\n');
    }

    Ok(content)
}

/// Slugify a title for use in filenames
fn slugify(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c
            } else if c.is_whitespace() {
                '-'
            } else {
                ' ' // Will be filtered out below
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-')
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("My Decision"), "my-decision");
        assert_eq!(slugify("Use Rust Framework"), "use-rust-framework");
        assert_eq!(slugify("  Leading Spaces  "), "leading-spaces");
        assert_eq!(slugify("Special-Characters!@#"), "special-characters");
    }
}
