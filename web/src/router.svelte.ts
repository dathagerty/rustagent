/**
 * Hash-based SPA router using Svelte 5 runes.
 * Provides simple routing without external dependencies.
 */

/**
 * Route definition with pattern and name.
 */
export type RouteDefinition = {
  pattern: string;
  name: string;
};

/**
 * Route match result with name and extracted parameters.
 */
export type RouteMatch = {
  name: string;
  params: Record<string, string>;
};

/**
 * Parse a route pattern into a regex and parameter names.
 * For example, `/goals/:goalId/tasks` becomes regex `/^\/goals\/([^/]+)\/tasks$/`
 * and paramNames = ['goalId'].
 *
 * Pure function, not dependent on $state.
 */
function parseRoutePattern(pattern: string): { regex: RegExp; paramNames: Array<string> } {
  const paramNames: Array<string> = [];
  let regexPattern = pattern;

  // Replace :paramName with a capture group and record the param name
  const paramRegex = /:([a-zA-Z_][a-zA-Z0-9_]*)/g;
  regexPattern = regexPattern.replace(paramRegex, (_, paramName: string) => {
    paramNames.push(paramName);
    return '([^/]+)'; // Match anything except /
  });

  // Escape special regex characters in the pattern (but keep the replaced groups)
  // Instead, build the regex more carefully
  regexPattern = '^' + regexPattern + '$';
  const regex = new RegExp(regexPattern);

  return { regex, paramNames };
}

/**
 * Match a path against a route definition and extract parameters.
 *
 * Pure function, not dependent on $state.
 * Can be unit tested directly.
 *
 * @param path The URL path to match (e.g., '/goals/ra-1234/tasks')
 * @param routes Array of route definitions
 * @returns A RouteMatch with the matched route name and extracted params, or not-found
 */
export function matchRoute(path: string, routes: Array<RouteDefinition>): RouteMatch {
  // Handle empty string as root
  const normalizedPath = path === '' ? '/' : path;

  for (const route of routes) {
    const { regex, paramNames } = parseRoutePattern(route.pattern);
    const match = normalizedPath.match(regex);

    if (match) {
      const params: Record<string, string> = {};
      for (let i = 0; i < paramNames.length; i++) {
        params[paramNames[i]] = match[i + 1];
      }
      return { name: route.name, params };
    }
  }

  // No match found
  return { name: 'not-found', params: {} };
}

/**
 * Router state object.
 * Managed as Svelte 5 $state for reactivity.
 */
export const routerState = $state<{
  path: string;
  params: Record<string, string>;
}>({
  path: '/',
  params: {},
});

/**
 * Route definitions for the application.
 */
const routes: Array<RouteDefinition> = [
  { pattern: '/', name: 'dashboard' },
  { pattern: '/projects', name: 'project-list' },
  { pattern: '/projects/:projectId', name: 'project-detail' },
  { pattern: '/goals/:goalId/tasks', name: 'task-tree' },
  { pattern: '/goals/:goalId/decisions', name: 'decision-graph' },
  { pattern: '/goals/:goalId/agents', name: 'agent-monitor' },
  { pattern: '/goals/:goalId/sessions', name: 'session-history' },
  { pattern: '/search', name: 'graph-search' },
];

/**
 * Get the current route based on the current path.
 * Returns a derived value computed from the current router state.
 */
export function getCurrentRoute(): RouteMatch {
  return matchRoute(routerState.path, routes);
}

/**
 * Navigate to a path by setting the hash.
 * @param path The path to navigate to (e.g., '/projects/ra-abcd')
 */
export function navigate(path: string): void {
  window.location.hash = '#' + path;
}

/**
 * Initialize the router and listen for hash changes.
 * The $effect handles setup and cleanup automatically.
 */
$effect(() => {
  const handler = () => {
    // Parse the hash and update routerState
    let hash = window.location.hash;
    // Remove leading '#'
    if (hash.startsWith('#')) {
      hash = hash.slice(1);
    }
    routerState.path = hash === '' ? '/' : hash;
  };

  // Parse current hash on init
  handler();

  // Listen for hash changes
  window.addEventListener('hashchange', handler);

  // Return cleanup function
  return () => {
    window.removeEventListener('hashchange', handler);
  };
});
