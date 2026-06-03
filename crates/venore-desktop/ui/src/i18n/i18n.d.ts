// =============================================================================
// i18n Type Augmentation — enables type-safe translation keys
// =============================================================================

import 'react-i18next'

import type enCommon from './locales/en/common.json'
import type enErrors from './locales/en/errors.json'
import type enMenu from './locales/en/menu.json'
import type enScreens from './locales/en/screens.json'
import type enWorkspace from './locales/en/workspace.json'
import type enChat from './locales/en/chat.json'
import type enProject from './locales/en/project.json'
import type enWizard from './locales/en/wizard.json'
import type enGithub from './locales/en/github.json'
import type enAgents from './locales/en/agents.json'
import type enSessions from './locales/en/sessions.json'

declare module 'react-i18next' {
  interface CustomTypeOptions {
    defaultNS: 'common'
    resources: {
      common: typeof enCommon
      errors: typeof enErrors
      menu: typeof enMenu
      screens: typeof enScreens
      workspace: typeof enWorkspace
      chat: typeof enChat
      project: typeof enProject
      wizard: typeof enWizard
      github: typeof enGithub
      agents: typeof enAgents
      sessions: typeof enSessions
    }
  }
}
