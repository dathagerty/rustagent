use super::{ApiError, AppState};
use crate::project::Project;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
    pub path: String,
}

#[derive(Serialize, Deserialize)]
pub struct ProjectResponse {
    pub id: String,
    pub name: String,
    pub path: String,
    pub registered_at: String,
}

impl From<Project> for ProjectResponse {
    fn from(p: Project) -> Self {
        Self {
            id: p.id,
            name: p.name,
            path: p.path.display().to_string(),
            registered_at: p.registered_at.to_rfc3339(),
        }
    }
}

/// GET /api/projects
pub async fn list_projects(
    State(state): State<AppState>,
) -> Result<Json<Vec<ProjectResponse>>, ApiError> {
    let projects = state.project_store.list().await?;
    Ok(Json(
        projects.into_iter().map(ProjectResponse::from).collect(),
    ))
}

/// POST /api/projects
pub async fn create_project(
    State(state): State<AppState>,
    Json(body): Json<CreateProjectRequest>,
) -> Result<(StatusCode, Json<ProjectResponse>), ApiError> {
    let path = std::path::Path::new(&body.path);
    let canonical = path
        .canonicalize()
        .map_err(|e| ApiError::BadRequest(format!("Invalid path '{}': {}", body.path, e)))?;

    match state.project_store.add(&body.name, &canonical).await {
        Ok(project) => Ok((StatusCode::CREATED, Json(ProjectResponse::from(project)))),
        Err(e) if e.to_string().contains("UNIQUE constraint") => Err(ApiError::Conflict(
            format!("Project '{}' already exists", body.name),
        )),
        Err(e) => Err(ApiError::Internal(e.to_string())),
    }
}

/// GET /api/projects/:id
pub async fn get_project(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ProjectResponse>, ApiError> {
    // Try by name first, then by ID
    let project = match state.project_store.get_by_name(&id).await? {
        Some(p) => Some(p),
        None => state.project_store.get_by_id(&id).await?,
    };
    match project {
        Some(p) => Ok(Json(ProjectResponse::from(p))),
        None => Err(ApiError::NotFound(format!("Project '{}' not found", id))),
    }
}

/// DELETE /api/projects/:id
pub async fn delete_project(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let removed = state.project_store.remove(&id).await?;
    if removed {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::NotFound(format!("Project '{}' not found", id)))
    }
}
