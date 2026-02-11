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
