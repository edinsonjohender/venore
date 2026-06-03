// =============================================================================
// SettingsModal — Preferences entry-point
// =============================================================================
// Thin shell: mounts the generic `SidebarModal` with the sections declared in
// `settings.config.ts`. State (open + active section) lives in
// `useSettingsStore` so anything in the app can open it via
// `useSettingsStore.getState().openModal('section-id')`.

import { SidebarModal, type SidebarModalConfig } from '../ui/SidebarModal'
import { useSettingsStore } from '@/stores/settingsStore'
import {
  SETTINGS_SECTIONS,
  DEFAULT_SETTINGS_SECTION,
  type SettingsSectionId,
} from './settings.config'

const SETTINGS_CONFIG: SidebarModalConfig<SettingsSectionId> = {
  title: 'Settings',
  subtitle: 'Configure your workspace',
  sections: SETTINGS_SECTIONS,
  defaultSection: DEFAULT_SETTINGS_SECTION,
  sidebarWidth: 220,
  maxWidth: 900,
  maxHeight: '700px',
}

export function SettingsModal() {
  const isOpen = useSettingsStore((s) => s.isOpen)
  const activeTab = useSettingsStore((s) => s.activeTab) as SettingsSectionId
  const setActiveTab = useSettingsStore((s) => s.setActiveTab)
  const closeModal = useSettingsStore((s) => s.closeModal)

  return (
    <SidebarModal
      isOpen={isOpen}
      onClose={closeModal}
      config={SETTINGS_CONFIG}
      activeSection={activeTab}
      onSectionChange={(id) => setActiveTab(id)}
    />
  )
}

export default SettingsModal
