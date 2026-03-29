/**
 * Date formatting utility functions.
 */

/**
 * Format a date string into human-readable format.
 * @param dateStr - ISO date string or any valid Date input
 * @returns Formatted date string (e.g., "Feb 11, 2026") or original string if parsing fails
 */
export function formatDate(dateStr: string): string {
  try {
    const date = new Date(dateStr);
    return date.toLocaleDateString('en-US', {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
    });
  } catch {
    return dateStr;
  }
}

/**
 * Format a date string into relative time format (e.g., "2s ago", "1m ago").
 * @param dateStr - ISO date string
 * @returns Relative time string or "unknown" if parsing fails
 */
export function formatRelativeTime(dateStr: string): string {
  try {
    const date = new Date(dateStr);
    const now = new Date();
    const seconds = Math.floor((now.getTime() - date.getTime()) / 1000);

    if (seconds < 60) {
      return `${seconds}s ago`;
    }

    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) {
      return `${minutes}m ago`;
    }

    const hours = Math.floor(minutes / 60);
    if (hours < 24) {
      return `${hours}h ago`;
    }

    const days = Math.floor(hours / 24);
    return `${days}d ago`;
  } catch {
    return 'unknown';
  }
}
