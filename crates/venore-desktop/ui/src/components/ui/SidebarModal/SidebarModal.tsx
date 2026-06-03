// =============================================================================
// SidebarModal — generic modal with left-nav + content panes
// =============================================================================
// Ported from v1 (src/components/ui/SidebarModal). Used by Settings; can be
// reused by any future "modal with multiple sub-screens" surface. Stays
// presentational — section state is either controlled (callers can sync
// with a store) or uncontrolled (internal `useState` keyed by config).

import { useState, useEffect } from 'react'
import { X, Settings } from 'lucide-react'
import type { SidebarModalProps } from './types'

export function SidebarModal<T extends string = string>({
  isOpen,
  onClose,
  config,
  activeSection: controlledSection,
  onSectionChange,
}: SidebarModalProps<T>) {
  const {
    title,
    subtitle,
    headerIcon,
    sections,
    defaultSection,
    sidebarWidth = 220,
    maxWidth = 900,
    maxHeight = '85vh',
  } = config

  const [internalSection, setInternalSection] = useState<T>(
    defaultSection || (sections[0]?.id as T),
  )

  const isControlled = controlledSection !== undefined && onSectionChange !== undefined
  const activeSection = isControlled ? controlledSection : internalSection

  const handleSectionChange = (section: T) => {
    if (isControlled) {
      onSectionChange?.(section)
    } else {
      setInternalSection(section)
    }
  }

  // Reset to default when the modal opens (uncontrolled only). Otherwise the
  // caller's state owns the section across opens.
  useEffect(() => {
    if (isOpen && !isControlled) {
      setInternalSection(defaultSection || (sections[0]?.id as T))
    }
  }, [isOpen, defaultSection, sections, isControlled])

  const activeSectionConfig = sections.find((s) => s.id === activeSection)
  const ActiveComponent = activeSectionConfig?.component

  if (!isOpen) return null

  return (
    <div className="fixed inset-0 z-[100] flex items-center justify-center">
      <div
        className="absolute inset-0 bg-black/60 backdrop-blur-sm"
        onClick={onClose}
      />

      <div className="relative z-10 w-full mx-4" style={{ maxWidth }}>
        <div
          className="bg-background border border-border rounded-xl shadow-2xl flex overflow-hidden"
          style={{ height: maxHeight, maxHeight }}
        >
          {/* Sidebar */}
          <div
            className="flex-shrink-0 bg-background-secondary border-r border-border flex flex-col"
            style={{ width: sidebarWidth }}
          >
            <div className="h-[60px] px-4 border-b border-border flex items-center">
              <div className="flex items-center gap-3">
                <div className="p-2 rounded-lg bg-background-tertiary text-foreground-muted">
                  {headerIcon || <Settings size={18} />}
                </div>
                <div className="min-w-0">
                  <h2 className="text-sm font-medium text-foreground truncate">{title}</h2>
                  {subtitle && (
                    <p className="text-xs text-foreground-muted truncate">{subtitle}</p>
                  )}
                </div>
              </div>
            </div>

            <nav className="flex-1 overflow-y-auto p-2">
              {sections.map((section) => {
                const Icon = section.icon
                const isActive = activeSection === section.id
                const isDisabled = section.disabled
                return (
                  <button
                    key={section.id}
                    onClick={() => !isDisabled && handleSectionChange(section.id)}
                    disabled={isDisabled}
                    title={section.description}
                    className={`
                      w-full flex items-center gap-3 px-3 py-2.5 rounded-lg text-left transition-colors mb-1
                      ${
                        isActive
                          ? 'bg-brand/10 text-brand'
                          : isDisabled
                            ? 'text-foreground-subtle cursor-not-allowed'
                            : 'text-foreground-muted hover:bg-background-tertiary hover:text-foreground'
                      }
                    `}
                  >
                    <Icon size={18} className={isActive ? 'text-brand' : ''} />
                    <span className="flex-1 text-sm truncate">{section.label}</span>
                    {section.badge && (
                      <span
                        className={`
                          px-1.5 py-0.5 text-[10px] font-medium rounded
                          ${
                            section.badgeVariant === 'success'
                              ? 'bg-semantic-success/10 text-semantic-success'
                              : section.badgeVariant === 'warning'
                                ? 'bg-semantic-warning/10 text-semantic-warning'
                                : section.badgeVariant === 'error'
                                  ? 'bg-semantic-error/10 text-semantic-error'
                                  : 'bg-brand/10 text-brand'
                          }
                        `}
                      >
                        {section.badge}
                      </span>
                    )}
                  </button>
                )
              })}
            </nav>
          </div>

          {/* Content pane */}
          <div className="flex-1 flex flex-col min-w-0">
            <div className="h-[60px] px-5 border-b border-border flex items-center justify-between">
              <div>
                <h3 className="text-sm font-medium text-foreground">
                  {activeSectionConfig?.label}
                </h3>
                {activeSectionConfig?.description && (
                  <p className="text-xs text-foreground-muted mt-0.5">
                    {activeSectionConfig.description}
                  </p>
                )}
              </div>
              <button
                onClick={onClose}
                className="p-1.5 rounded-md border border-border hover:border-border-hover hover:bg-background-tertiary text-foreground-muted hover:text-foreground transition-colors"
                aria-label="Close"
              >
                <X size={16} />
              </button>
            </div>

            <div className="flex-1 overflow-y-auto p-5">
              {ActiveComponent && <ActiveComponent />}
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}

export default SidebarModal
