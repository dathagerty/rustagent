/**
 * Reactive store for project state using Svelte 5 runes.
 * Manages the list of projects and selected project.
 */

import { createApiClient } from '../api/client';
import type { ProjectResponse } from '../types';

/**
 * Projects state object.
 * Managed as Svelte 5 $state for reactivity.
 */
export const projectsState = $state<{
  projects: Array<ProjectResponse>;
  selectedProjectId: string | null;
  loading: boolean;
  error: string | null;
}>({
  projects: [],
  selectedProjectId: null,
  loading: false,
  error: null,
});

/**
 * Derived: the currently selected project, or null if none selected.
 */
export const selectedProject = $derived.by(() => {
  if (!projectsState.selectedProjectId) {
    return null;
  }
  return (
    projectsState.projects.find((p) => p.id === projectsState.selectedProjectId) || null
  );
});

const apiClient = createApiClient();

/**
 * Load all projects from the API.
 */
export async function loadProjects(): Promise<void> {
  projectsState.loading = true;
  projectsState.error = null;
  try {
    projectsState.projects = await apiClient.listProjects();
  } catch (error) {
    projectsState.error = error instanceof Error ? error.message : 'Unknown error';
  } finally {
    projectsState.loading = false;
  }
}

/**
 * Select a project by ID.
 */
export function selectProject(id: string): void {
  projectsState.selectedProjectId = id;
}

/**
 * Create a new project.
 */
export async function createProject(name: string, path: string): Promise<void> {
  projectsState.loading = true;
  projectsState.error = null;
  try {
    const created = await apiClient.createProject({ name, path });
    projectsState.projects.push(created);
  } catch (error) {
    projectsState.error = error instanceof Error ? error.message : 'Unknown error';
  } finally {
    projectsState.loading = false;
  }
}

/**
 * Remove a project by ID.
 */
export async function removeProject(id: string): Promise<void> {
  projectsState.loading = true;
  projectsState.error = null;
  try {
    await apiClient.deleteProject(id);
    projectsState.projects = projectsState.projects.filter((p) => p.id !== id);
    if (projectsState.selectedProjectId === id) {
      projectsState.selectedProjectId = null;
    }
  } catch (error) {
    projectsState.error = error instanceof Error ? error.message : 'Unknown error';
  } finally {
    projectsState.loading = false;
  }
}
