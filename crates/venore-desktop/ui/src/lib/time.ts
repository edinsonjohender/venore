// =============================================================================
// Shared time utilities — formatTimeAgo with i18n support
// =============================================================================

import i18n from '@/i18n'

/**
 * Format a date/timestamp as a human-readable relative time string.
 * Uses i18n common.timeAgo keys for localization.
 */
export function formatTimeAgo(date: Date | string | number): string {
  const timestamp = typeof date === 'number'
    ? date
    : new Date(date).getTime()

  const now = Date.now()
  const diff = now - timestamp
  const seconds = Math.floor(diff / 1000)
  const minutes = Math.floor(seconds / 60)
  const hours = Math.floor(minutes / 60)
  const days = Math.floor(hours / 24)
  const weeks = Math.floor(days / 7)
  const months = Math.floor(days / 30)

  if (months > 0) {
    return i18n.t('timeAgo.monthsAgo', { count: months })
  } else if (weeks > 0) {
    return i18n.t('timeAgo.weeksAgo', { count: weeks })
  } else if (days > 0) {
    return i18n.t('timeAgo.daysAgo', { count: days })
  } else if (hours > 0) {
    return i18n.t('timeAgo.hoursAgo', { count: hours })
  } else if (minutes > 0) {
    return i18n.t('timeAgo.minutesAgo', { count: minutes })
  } else {
    return i18n.t('timeAgo.justNow')
  }
}
