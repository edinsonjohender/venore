// =============================================================================
// LanguageSelector — Language switching UI pieces
// =============================================================================
// Provides: LanguageMenuItems (for TitleBar submenu), LanguageIndicator (for StatusBar)

import { useTranslation } from 'react-i18next'
import { Globe } from 'lucide-react'
import { SUPPORTED_LANGUAGES, STORAGE_KEY } from '@/i18n/constants'
import type { LanguageCode } from '@/i18n/constants'
import {
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
} from './ui/dropdown-menu'

// -----------------------------------------------------------------------------
// Shared logic
// -----------------------------------------------------------------------------

function useLanguageSwitch() {
  const { i18n } = useTranslation()

  const changeLanguage = (code: LanguageCode) => {
    i18n.changeLanguage(code)
    localStorage.setItem(STORAGE_KEY, code)
  }

  return { currentLanguage: i18n.language, changeLanguage }
}

// -----------------------------------------------------------------------------
// LanguageMenuItems — for use inside a DropdownMenuSubContent
// -----------------------------------------------------------------------------

export function LanguageMenuItems() {
  const { currentLanguage, changeLanguage } = useLanguageSwitch()

  return (
    <DropdownMenuRadioGroup value={currentLanguage} onValueChange={(v) => changeLanguage(v as LanguageCode)}>
      {SUPPORTED_LANGUAGES.map((lang) => (
        <DropdownMenuRadioItem
          key={lang.code}
          value={lang.code}
          className="text-xs px-3 py-1.5 pl-8 rounded-sm text-foreground-muted cursor-default select-none outline-none focus:bg-background-secondary focus:text-foreground"
        >
          {lang.nativeName}
        </DropdownMenuRadioItem>
      ))}
    </DropdownMenuRadioGroup>
  )
}

// -----------------------------------------------------------------------------
// LanguageIndicator — compact indicator for StatusBar
// -----------------------------------------------------------------------------

export function LanguageIndicator() {
  const { currentLanguage } = useLanguageSwitch()

  return (
    <span className="flex items-center gap-1">
      <Globe className="w-3.5 h-3.5" />
      <span>{currentLanguage.toUpperCase()}</span>
    </span>
  )
}
