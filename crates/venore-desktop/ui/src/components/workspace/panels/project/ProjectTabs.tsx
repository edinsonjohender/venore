// =============================================================================
// ProjectTabs - Context / Files tab switcher
// =============================================================================

import { useTranslation } from 'react-i18next'
import { BrainCircuit, FolderOpen } from 'lucide-react'
import { cn } from '@/lib/utils'

type TabId = 'context' | 'files'

interface ProjectTabsProps {
  active: TabId
  onChange: (tab: TabId) => void
}

const TABS: { id: TabId; labelKey: string; icon: typeof BrainCircuit }[] = [
  { id: 'context', labelKey: 'tabs.context', icon: BrainCircuit },
  { id: 'files', labelKey: 'tabs.files', icon: FolderOpen },
]

export function ProjectTabs({ active, onChange }: ProjectTabsProps) {
  const { t } = useTranslation('project')

  return (
    <div className="flex shrink-0 border-b border-border">
      {TABS.map(({ id, labelKey, icon: Icon }) => (
        <button
          key={id}
          onClick={() => onChange(id)}
          className={cn(
            'flex-1 flex items-center justify-center gap-1.5 h-8 text-xs font-medium',
            'border-b-2 transition-colors',
            active === id
              ? 'text-brand border-brand'
              : 'text-foreground-muted border-transparent hover:text-foreground',
          )}
        >
          <Icon className="w-3.5 h-3.5" />
          {t(labelKey)}
        </button>
      ))}
    </div>
  )
}
