import { describe, it, expect } from 'vitest';
import { matchRoute } from './router.svelte';


describe('matchRoute', () => {
  const routes = [
    { pattern: '/', name: 'dashboard' },
    { pattern: '/projects', name: 'project-list' },
    { pattern: '/projects/:projectId', name: 'project-detail' },
    { pattern: '/goals/:goalId/tasks', name: 'task-tree' },
    { pattern: '/goals/:goalId/decisions', name: 'decision-graph' },
    { pattern: '/goals/:goalId/agents', name: 'agent-monitor' },
    { pattern: '/goals/:goalId/sessions', name: 'session-history' },
    { pattern: '/search', name: 'graph-search' },
  ];

  it('matches root path to dashboard', () => {
    const result = matchRoute('/', routes);
    expect(result.name).toBe('dashboard');
    expect(result.params).toEqual({});
  });

  it('matches /projects to project-list', () => {
    const result = matchRoute('/projects', routes);
    expect(result.name).toBe('project-list');
    expect(result.params).toEqual({});
  });

  it('matches /projects/:projectId with parameter extraction', () => {
    const result = matchRoute('/projects/ra-abcd', routes);
    expect(result.name).toBe('project-detail');
    expect(result.params).toEqual({ projectId: 'ra-abcd' });
  });

  it('matches /goals/:goalId/tasks with parameter extraction', () => {
    const result = matchRoute('/goals/ra-1234/tasks', routes);
    expect(result.name).toBe('task-tree');
    expect(result.params).toEqual({ goalId: 'ra-1234' });
  });

  it('matches /goals/:goalId/decisions with parameter extraction', () => {
    const result = matchRoute('/goals/ra-1234/decisions', routes);
    expect(result.name).toBe('decision-graph');
    expect(result.params).toEqual({ goalId: 'ra-1234' });
  });

  it('matches /goals/:goalId/agents with parameter extraction', () => {
    const result = matchRoute('/goals/ra-1234/agents', routes);
    expect(result.name).toBe('agent-monitor');
    expect(result.params).toEqual({ goalId: 'ra-1234' });
  });

  it('matches /goals/:goalId/sessions with parameter extraction', () => {
    const result = matchRoute('/goals/ra-1234/sessions', routes);
    expect(result.name).toBe('session-history');
    expect(result.params).toEqual({ goalId: 'ra-1234' });
  });

  it('matches /search to graph-search', () => {
    const result = matchRoute('/search', routes);
    expect(result.name).toBe('graph-search');
    expect(result.params).toEqual({});
  });

  it('returns not-found for unknown path', () => {
    const result = matchRoute('/unknown/path', routes);
    expect(result.name).toBe('not-found');
    expect(result.params).toEqual({});
  });

  it('returns dashboard for empty string (root route)', () => {
    const result = matchRoute('', routes);
    expect(result.name).toBe('dashboard');
    expect(result.params).toEqual({});
  });

  it('extracts multiple parameters from complex paths', () => {
    const customRoutes = [
      { pattern: '/projects/:projectId/goals/:goalId', name: 'goal-detail' },
    ];
    const result = matchRoute('/projects/proj-123/goals/goal-456', customRoutes);
    expect(result.name).toBe('goal-detail');
    expect(result.params).toEqual({ projectId: 'proj-123', goalId: 'goal-456' });
  });

  it('does not match partial paths', () => {
    const result = matchRoute('/goal', routes);
    expect(result.name).toBe('not-found');
  });

  it('handles trailing slashes correctly', () => {
    const result = matchRoute('/projects/', routes);
    // /projects/ should not match /projects
    expect(result.name).toBe('not-found');
  });

  it('handles parameters with special characters', () => {
    const result = matchRoute('/projects/ra-abc-def', routes);
    expect(result.name).toBe('project-detail');
    expect(result.params).toEqual({ projectId: 'ra-abc-def' });
  });
});
