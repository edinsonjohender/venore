// =============================================================================
// Error Messages — maps backend error codes to user-friendly messages via i18n
// =============================================================================

import i18n from '@/i18n'

export interface ErrorInfo {
  message: string
  suggestion?: string
}

/**
 * Get a user-friendly error message for a backend error code.
 * Falls back to the raw backend message if no mapping exists.
 */
export function getFriendlyErrorMessage(code: string, fallback: string): string {
  const key = `${code}.message`
  const translated = i18n.t(key, { ns: 'errors', defaultValue: '' })
  return translated || fallback
}

/**
 * Get full error info (message + optional suggestion) for a backend error code.
 * Falls back to the raw backend message if no mapping exists.
 */
export function getErrorInfo(code: string, fallback: string): ErrorInfo {
  const messageKey = `${code}.message`
  const suggestionKey = `${code}.suggestion`

  const message = i18n.t(messageKey, { ns: 'errors', defaultValue: '' })

  if (!message) {
    return { message: fallback }
  }

  const suggestion = i18n.t(suggestionKey, { ns: 'errors', defaultValue: '' })
  return suggestion
    ? { message, suggestion }
    : { message }
}
