/**
 * Shared API client instance for the entire application.
 * Export a singleton ApiClient so all stores use the same instance.
 */

import { ApiClient } from './client';

export const apiClient = new ApiClient();
