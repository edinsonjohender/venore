// =============================================================================
// SkillPalette - Floating dropdown for slash command selection
// =============================================================================
// Appears when user types "/" at the start of input. Shows available skills
// with keyboard navigation (Up/Down/Enter/Escape).

import { useState, useEffect, useCallback, useRef } from 'react'
import { Zap } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { SkillDto } from '@/lib/tauri'

interface SkillPaletteProps {
  skills: SkillDto[]
  filter: string // text after "/" to filter
  onSelect: (skill: SkillDto) => void
  onClose: () => void
}

export function SkillPalette({ skills, filter, onSelect, onClose }: SkillPaletteProps) {
  const [selectedIndex, setSelectedIndex] = useState(0)
  const listRef = useRef<HTMLDivElement>(null)

  const filtered = skills.filter((s) =>
    s.name.toLowerCase().includes(filter.toLowerCase()),
  )

  // Reset selection when filter changes
  useEffect(() => {
    setSelectedIndex(0)
  }, [filter])

  // Keyboard navigation
  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (filtered.length === 0) return

      switch (e.key) {
        case 'ArrowDown':
          e.preventDefault()
          setSelectedIndex((i) => (i + 1) % filtered.length)
          break
        case 'ArrowUp':
          e.preventDefault()
          setSelectedIndex((i) => (i - 1 + filtered.length) % filtered.length)
          break
        case 'Enter':
        case 'Tab':
          e.preventDefault()
          if (filtered[selectedIndex]) onSelect(filtered[selectedIndex])
          break
        case 'Escape':
          e.preventDefault()
          onClose()
          break
      }
    },
    [filtered, selectedIndex, onSelect, onClose],
  )

  useEffect(() => {
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [handleKeyDown])

  // Scroll selected into view
  useEffect(() => {
    const el = listRef.current?.children[selectedIndex] as HTMLElement | undefined
    el?.scrollIntoView({ block: 'nearest' })
  }, [selectedIndex])

  if (filtered.length === 0) return null

  return (
    <div
      ref={listRef}
      className="absolute bottom-full left-0 right-0 mb-1 mx-2 max-h-[200px] overflow-y-auto rounded-lg border border-border bg-background-secondary shadow-lg z-50"
    >
      {filtered.map((skill, i) => (
        <button
          key={skill.name}
          type="button"
          onClick={() => onSelect(skill)}
          className={cn(
            'w-full flex items-center gap-2.5 px-3 py-2 text-left transition-colors',
            i === selectedIndex
              ? 'bg-brand/10 text-foreground'
              : 'text-foreground-muted hover:bg-background-tertiary',
          )}
        >
          <Zap className="w-3.5 h-3.5 text-amber-400 shrink-0" />
          <div className="flex flex-col min-w-0">
            <span className="text-xs font-mono font-medium">/{skill.name}</span>
            <span className="text-[10px] text-foreground-subtle truncate">
              {skill.description}
            </span>
          </div>
        </button>
      ))}
    </div>
  )
}
