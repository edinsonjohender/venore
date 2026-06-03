/**
 * Frontend logging utility
 *
 * Provides structured logging with levels and domains.
 * In production, only warn and error levels are logged.
 */

export type LogLevel = 'debug' | 'info' | 'warn' | 'error'

interface LogConfig {
  level: LogLevel
  enabled: boolean
}

const config: LogConfig = {
  level: import.meta.env.DEV ? 'debug' : 'warn',
  enabled: true,
}

const levelPriority: Record<LogLevel, number> = {
  debug: 0,
  info: 1,
  warn: 2,
  error: 3,
}

function shouldLog(level: LogLevel): boolean {
  if (!config.enabled) return false
  return levelPriority[level] >= levelPriority[config.level]
}

function formatMessage(domain: string, message: string): string {
  return `[${domain}] ${message}`
}

export const logger = {
  debug(domain: string, message: string, ...args: unknown[]): void {
    if (shouldLog('debug')) {
      console.log(formatMessage(domain, message), ...args)
    }
  },

  info(domain: string, message: string, ...args: unknown[]): void {
    if (shouldLog('info')) {
      console.log(formatMessage(domain, message), ...args)
    }
  },

  warn(domain: string, message: string, ...args: unknown[]): void {
    if (shouldLog('warn')) {
      console.warn(formatMessage(domain, message), ...args)
    }
  },

  error(domain: string, message: string, ...args: unknown[]): void {
    if (shouldLog('error')) {
      console.error(formatMessage(domain, message), ...args)
    }
  },

  /** Configure logging level */
  setLevel(level: LogLevel): void {
    config.level = level
  },

  /** Enable/disable all logging */
  setEnabled(enabled: boolean): void {
    config.enabled = enabled
  },
}

/**
 * Create a domain-specific logger
 *
 * @example
 * const log = createLogger('wizard')
 * log.info('Starting generation')
 * log.error('Failed to load', error)
 */
export function createLogger(domain: string) {
  return {
    debug: (message: string, ...args: unknown[]) => logger.debug(domain, message, ...args),
    info: (message: string, ...args: unknown[]) => logger.info(domain, message, ...args),
    warn: (message: string, ...args: unknown[]) => logger.warn(domain, message, ...args),
    error: (message: string, ...args: unknown[]) => logger.error(domain, message, ...args),
  }
}
