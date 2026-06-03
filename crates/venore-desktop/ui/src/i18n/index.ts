// =============================================================================
// i18n Initialization
// =============================================================================
// EN is bundled statically (zero FOUC). Other languages load lazily via Vite
// dynamic imports. Preference persisted in localStorage.

import i18n from 'i18next'
import { initReactI18next } from 'react-i18next'
import { STORAGE_KEY, DEFAULT_LANGUAGE, DEFAULT_NAMESPACE, NAMESPACES } from './constants'

import enCommon from './locales/en/common.json'
import enErrors from './locales/en/errors.json'
import enMenu from './locales/en/menu.json'
import enScreens from './locales/en/screens.json'
import enWorkspace from './locales/en/workspace.json'
import enChat from './locales/en/chat.json'
import enProject from './locales/en/project.json'
import enWizard from './locales/en/wizard.json'
import enGithub from './locales/en/github.json'
import enAgents from './locales/en/agents.json'
import enSessions from './locales/en/sessions.json'
import enUpdater from './locales/en/updater.json'

// -----------------------------------------------------------------------------
// Lazy-loading backend (inline plugin — no extra dependency)
// -----------------------------------------------------------------------------

const LazyImportBackend = {
  type: 'backend' as const,
  read(language: string, namespace: string, callback: (err: Error | null, data?: Record<string, unknown>) => void) {
    // EN is already bundled — skip loading
    if (language === 'en') {
      callback(null, {})
      return
    }

    import(`./locales/${language}/${namespace}.json`)
      .then((mod) => callback(null, mod.default ?? mod))
      .catch(() => callback(null, {})) // Missing locale file → fallback to EN
  },
}

// -----------------------------------------------------------------------------
// Read saved preference
// -----------------------------------------------------------------------------

function getSavedLanguage(): string {
  try {
    return localStorage.getItem(STORAGE_KEY) ?? DEFAULT_LANGUAGE
  } catch {
    return DEFAULT_LANGUAGE
  }
}

// -----------------------------------------------------------------------------
// Init
// -----------------------------------------------------------------------------

i18n
  .use(LazyImportBackend)
  .use(initReactI18next)
  .init({
    lng: getSavedLanguage(),
    fallbackLng: DEFAULT_LANGUAGE,
    defaultNS: DEFAULT_NAMESPACE,
    ns: [...NAMESPACES],

    // EN bundled resources
    partialBundledLanguages: true,
    resources: {
      en: {
        common: enCommon,
        errors: enErrors,
        menu: enMenu,
        screens: enScreens,
        workspace: enWorkspace,
        chat: enChat,
        project: enProject,
        wizard: enWizard,
        github: enGithub,
        agents: enAgents,
        sessions: enSessions,
        updater: enUpdater,
      },
    },

    interpolation: {
      escapeValue: false, // React handles XSS
    },

    react: {
      useSuspense: false, // Tauri doesn't need SSR/Suspense
    },

    returnNull: false,
    returnEmptyString: false,
  })

export default i18n
