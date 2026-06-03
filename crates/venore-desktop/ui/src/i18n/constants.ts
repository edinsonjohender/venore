// =============================================================================
// i18n Constants
// =============================================================================

export const STORAGE_KEY = 'venore-language'

export const SUPPORTED_LANGUAGES = [
  { code: 'en', name: 'English', nativeName: 'English' },
  { code: 'es', name: 'Spanish', nativeName: 'Español' },
  { code: 'zh', name: 'Chinese', nativeName: '中文' },
  { code: 'pt', name: 'Portuguese', nativeName: 'Português' },
  { code: 'ja', name: 'Japanese', nativeName: '日本語' },
] as const

export type LanguageCode = (typeof SUPPORTED_LANGUAGES)[number]['code']

export const DEFAULT_LANGUAGE: LanguageCode = 'en'

export const NAMESPACES = [
  'common',
  'errors',
  'menu',
  'screens',
  'workspace',
  'chat',
  'project',
  'wizard',
  'github',
  'agents',
  'sessions',
  'updater',
] as const

export const DEFAULT_NAMESPACE = 'common' as const
