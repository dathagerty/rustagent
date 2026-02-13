use crate::daemon::DaemonConfig;
use crate::daemon::api::projects::ProjectResponse;
use crate::graph::GraphNode;
use anyhow::Result;
use reqwest::Client;
use serde::de::DeserializeOwned;

/// HTTP client for communicating with a running daemon
#[derive(Clone)]
pub struct DaemonClient {
    client: Client,
    base_url: String,
}

impl DaemonClient {
    pub fn new(config: &DaemonConfig) -> Self {
        Self {
            client: Client::new(),
            base_url: format!("http://{}:{}", config.bind_address, config.port),
        }
    }

    /// Check if the daemon is healthy
    pub async fn health(&self) -> bool {
        match self
            .client
            .get(format!("{}/api/health", self.base_url))
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await
        {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }

    /// Generic GET request returning parsed JSON
    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.client.get(&url).send().await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("API error ({}): {}", status.as_u16(), body);
        }
        Ok(resp.json().await?)
    }

    /// Generic POST request with JSON body returning parsed JSON
    pub async fn post<T: DeserializeOwned, B: serde::Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.client.post(&url).json(body).send().await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("API error ({}): {}", status.as_u16(), body);
        }
        Ok(resp.json().await?)
    }

    /// Generic DELETE request
    pub async fn delete(&self, path: &str) -> Result<()> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.client.delete(&url).send().await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("API error ({}): {}", status.as_u16(), body);
        }
        Ok(())
    }

    // ===== Project operations =====

    pub async fn projects_list(&self) -> Result<Vec<ProjectResponse>> {
        self.get("/api/projects").await
    }

    pub async fn project_add(&self, name: &str, path: &str) -> Result<ProjectResponse> {
        self.post(
            "/api/projects",
            &serde_json::json!({ "name": name, "path": path }),
        )
        .await
    }

    pub async fn project_get(&self, name: &str) -> Result<ProjectResponse> {
        self.get(&format!("/api/projects/{}", name)).await
    }

    pub async fn project_remove(&self, name: &str) -> Result<()> {
        self.delete(&format!("/api/projects/{}", name)).await
    }

    // ===== Goal operations =====

    pub async fn goals_list(&self, project_id: &str) -> Result<Vec<GraphNode>> {
        self.get(&format!("/api/projects/{}/goals", project_id))
            .await
    }

    // ===== Task operations =====

    pub async fn tasks_list(&self, goal_id: &str) -> Result<Vec<GraphNode>> {
        self.get(&format!("/api/goals/{}/tasks", goal_id)).await
    }

    pub async fn tasks_ready(&self, goal_id: &str) -> Result<Vec<GraphNode>> {
        self.get(&format!("/api/goals/{}/tasks/ready", goal_id))
            .await
    }

    pub async fn tasks_next(&self, goal_id: &str) -> Result<Option<GraphNode>> {
        self.get(&format!("/api/goals/{}/tasks/next", goal_id))
            .await
    }

    // ===== Search =====

    pub async fn search(&self, project_id: &str, query: &str) -> Result<Vec<GraphNode>> {
        self.post(
            &format!("/api/projects/{}/search", project_id),
            &serde_json::json!({ "query": query }),
        )
        .await
    }
}

/// Detect if a daemon is running and return a client for it.
/// Checks PID file first (fast), then confirms with HTTP health check (accurate).
pub async fn detect_daemon(config: &DaemonConfig) -> Option<DaemonClient> {
    // Fast check: PID file exists and process is alive
    if !crate::daemon::is_daemon_running(config).unwrap_or(false) {
        return None;
    }

    // Accurate check: HTTP health endpoint responds
    let client = DaemonClient::new(config);
    if client.health().await {
        Some(client)
    } else {
        None
    }
}
