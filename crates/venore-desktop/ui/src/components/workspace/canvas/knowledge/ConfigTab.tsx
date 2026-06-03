// =============================================================================
// ConfigTab — Seed, objective, parameters (editable forms)
// =============================================================================

import { useState, useEffect, useCallback } from 'react'
import { Upload, FileText, File, Image, X, FileSpreadsheet, FolderGit2, Check, Save } from 'lucide-react'
import { cn } from '@/lib/utils'
import { toast } from 'sonner'
import { tauriApi } from '@/lib/tauri'
import type { Feature, ResearchObjective, ResearchIntensity } from './mock-data'

interface ConfigTabProps {
  feature: Feature
  onSaved?: () => void
}

// Objective types for the select
const OBJECTIVES = [
  { value: 'validate', label: 'Validate feasibility' },
  { value: 'understand', label: 'Understand problem space' },
  { value: 'compare', label: 'Compare alternatives' },
  { value: 'decide', label: 'Make GO / NO-GO decision' },
  { value: 'explore', label: 'Open exploration' },
] as const

const INTENSITIES = [
  { value: 'shallow', label: 'Shallow', desc: 'Quick scan, 1-2 sources per point' },
  { value: 'moderate', label: 'Moderate', desc: 'Balanced depth, 3-5 sources per point' },
  { value: 'deep', label: 'Deep', desc: 'Exhaustive research, 5+ sources, cross-validation' },
] as const

const PRIORITIES = ['low', 'medium', 'high', 'critical'] as const

export function ConfigTab({ feature, onSaved }: ConfigTabProps) {
  // Local form state — initialized from feature props
  const [name, setName] = useState(feature.name)
  const [description, setDescription] = useState(feature.description)
  const [objective, setObjective] = useState(feature.objective)
  const [intensity, setIntensity] = useState<ResearchIntensity>(feature.intensity)
  const [priority, setPriority] = useState<string>(feature.priority)
  const [maxHexagons, setMaxHexagons] = useState(feature.maxHexagonsPerPhase)
  const [autoAdvance, setAutoAdvance] = useState(feature.autoAdvance)
  const [tags, setTags] = useState(feature.tags.join(', '))
  const [saving, setSaving] = useState(false)

  // Reset form when feature changes
  useEffect(() => {
    setName(feature.name)
    setDescription(feature.description)
    setObjective(feature.objective)
    setIntensity(feature.intensity)
    setPriority(feature.priority)
    setMaxHexagons(feature.maxHexagonsPerPhase)
    setAutoAdvance(feature.autoAdvance)
    setTags(feature.tags.join(', '))
  }, [feature.id])  // eslint-disable-line react-hooks/exhaustive-deps -- intentional: reset form only on feature switch, not on individual field changes

  // Dirty detection
  const isDirty = name !== feature.name
    || description !== feature.description
    || objective !== feature.objective
    || intensity !== feature.intensity
    || priority !== feature.priority
    || maxHexagons !== feature.maxHexagonsPerPhase
    || autoAdvance !== feature.autoAdvance
    || tags !== feature.tags.join(', ')

  const handleSave = async () => {
    if (saving) return
    setSaving(true)
    try {
      await tauriApi.updateKnowledgeFeature({
        id: feature.id,
        name,
        description,
        status: feature.status,
        priority,
        objective,
        intensity,
        maxHexagonsPerPhase: maxHexagons,
        autoAdvance,
        tags: JSON.stringify(tags.split(',').map((t) => t.trim()).filter(Boolean)),
      })
      onSaved?.()
      toast.success('Configuration saved')
    } catch (err) {
      toast.error('Failed to save configuration')
    } finally {
      setSaving(false)
    }
  }
  const [files, setFiles] = useState<{ name: string; size: number; type: string }[]>([])
  const [isDragOver, setIsDragOver] = useState(false)
  const [connectedProjects, setConnectedProjects] = useState<string[]>([])

  // Mock registered projects (will come from backend)
  const availableProjects = [
    { id: 'proj-1', name: 'venore-core', path: 'D:/development/project-context/venore_v2', type: 'code' as const, stack: 'Rust, SQLite, tree-sitter' },
    { id: 'proj-2', name: 'venore-desktop', path: 'D:/development/project-context/venore_v2/crates/venore-desktop', type: 'code' as const, stack: 'Tauri, React, TypeScript' },
    { id: 'proj-3', name: 'api-gateway', path: 'D:/development/api-gateway', type: 'code' as const, stack: 'Node.js, Express, PostgreSQL' },
  ]

  const toggleProject = (id: string) => {
    setConnectedProjects((prev) =>
      prev.includes(id) ? prev.filter((p) => p !== id) : [...prev, id],
    )
  }

  const handleDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    setIsDragOver(false)
    const dropped = Array.from(e.dataTransfer.files).map((f) => ({
      name: f.name,
      size: f.size,
      type: f.type || 'unknown',
    }))
    setFiles((prev) => [...prev, ...dropped])
  }, [])

  const handleFileInput = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    if (!e.target.files) return
    const selected = Array.from(e.target.files).map((f) => ({
      name: f.name,
      size: f.size,
      type: f.type || 'unknown',
    }))
    setFiles((prev) => [...prev, ...selected])
    e.target.value = ''
  }, [])

  const removeFile = (index: number) => {
    setFiles((prev) => prev.filter((_, i) => i !== index))
  }

  const formatSize = (bytes: number) => {
    if (bytes < 1024) return `${bytes} B`
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
  }

  const getFileIcon = (type: string) => {
    if (type.includes('pdf')) return FileText
    if (type.includes('image')) return Image
    if (type.includes('spreadsheet') || type.includes('csv') || type.includes('excel')) return FileSpreadsheet
    return File
  }

  return (
    <div className="flex-1 overflow-y-auto">
      <div className="max-w-2xl mx-auto p-6 space-y-6">

        {/* Seed */}
        <section className="space-y-3">
          <h3 className="text-xs font-medium text-foreground-muted uppercase tracking-wider">Seed</h3>
          <div className="space-y-3">
            <div className="space-y-1.5">
              <label className="text-[11px] text-foreground-muted">Name</label>
              <input
                type="text"
                value={name}
                onChange={(e) => setName(e.target.value)}
                className="w-full rounded-md border border-border/50 bg-background-secondary px-3 py-1.5 text-sm text-foreground outline-none focus:border-brand transition-colors"
              />
            </div>
            <div className="space-y-1.5">
              <label className="text-[11px] text-foreground-muted">Description</label>
              <textarea
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                rows={3}
                className="w-full rounded-md border border-border/50 bg-background-secondary px-3 py-1.5 text-sm text-foreground outline-none focus:border-brand transition-colors resize-none"
              />
            </div>
            <div className="space-y-1.5">
              <label className="text-[11px] text-foreground-muted">Tags</label>
              <input
                type="text"
                value={tags}
                onChange={(e) => setTags(e.target.value)}
                placeholder="e.g. crdt, collaboration, mesh"
                className="w-full rounded-md border border-border/50 bg-background-secondary px-3 py-1.5 text-sm text-foreground outline-none focus:border-brand transition-colors"
              />
            </div>
          </div>
        </section>

        {/* Objective */}
        <section className="space-y-3">
          <h3 className="text-xs font-medium text-foreground-muted uppercase tracking-wider">Objective</h3>
          <div className="space-y-1.5">
            <label className="text-[11px] text-foreground-muted">What do you want to achieve?</label>
            <select
              value={objective}
              onChange={(e) => setObjective(e.target.value as ResearchObjective)}
              className="w-full rounded-md border border-border/50 bg-background-secondary px-3 py-1.5 text-sm text-foreground outline-none focus:border-brand transition-colors"
            >
              {OBJECTIVES.map((o) => (
                <option key={o.value} value={o.value}>{o.label}</option>
              ))}
            </select>
          </div>
          <div className="space-y-1.5">
            <label className="text-[11px] text-foreground-muted">Priority</label>
            <div className="flex gap-2">
              {PRIORITIES.map((p) => (
                <button
                  key={p}
                  onClick={() => setPriority(p)}
                  className={cn(
                    'px-3 py-1 rounded-md text-xs font-medium capitalize transition-colors',
                    priority === p
                      ? 'bg-brand/15 text-brand border border-brand/30'
                      : 'bg-background-secondary text-foreground-muted border border-border/50 hover:text-foreground',
                  )}
                >
                  {p}
                </button>
              ))}
            </div>
          </div>
        </section>

        {/* Reference Materials */}
        <section className="space-y-3">
          <h3 className="text-xs font-medium text-foreground-muted uppercase tracking-wider">Reference Materials</h3>

          {/* Drop zone */}
          <div
            onDragOver={(e) => { e.preventDefault(); setIsDragOver(true) }}
            onDragLeave={() => setIsDragOver(false)}
            onDrop={handleDrop}
            className={cn(
              'rounded-md border-2 border-dashed px-4 py-6 text-center transition-colors',
              isDragOver
                ? 'border-brand bg-brand/5'
                : 'border-border/50 hover:border-border',
            )}
          >
            <Upload className="w-5 h-5 mx-auto mb-2 text-foreground-subtle" />
            <p className="text-[11px] text-foreground-muted mb-1">
              Drop files here or{' '}
              <label className="text-brand cursor-pointer hover:underline">
                browse
                <input
                  type="file"
                  multiple
                  onChange={handleFileInput}
                  className="hidden"
                  accept=".pdf,.doc,.docx,.txt,.md,.csv,.xlsx,.png,.jpg,.jpeg,.svg"
                />
              </label>
            </p>
            <p className="text-[9px] text-foreground-subtle">PDF, DOC, TXT, MD, CSV, XLSX, images</p>
          </div>

          {/* File list */}
          {files.length > 0 && (
            <div className="space-y-1">
              {files.map((file, i) => {
                const Icon = getFileIcon(file.type)
                return (
                  <div
                    key={`${file.name}-${i}`}
                    className="flex items-center gap-2 rounded-md bg-background-secondary border border-border/50 px-3 py-1.5"
                  >
                    <Icon className="w-3.5 h-3.5 text-foreground-muted shrink-0" />
                    <span className="text-[11px] text-foreground flex-1 truncate">{file.name}</span>
                    <span className="text-[9px] text-foreground-subtle shrink-0">{formatSize(file.size)}</span>
                    <button
                      onClick={() => removeFile(i)}
                      className="shrink-0 p-0.5 rounded hover:bg-red-500/10 text-foreground-subtle hover:text-red-400 transition-colors"
                    >
                      <X className="w-3 h-3" />
                    </button>
                  </div>
                )
              })}
            </div>
          )}
        </section>

        {/* Connected Projects */}
        <section className="space-y-3">
          <h3 className="text-xs font-medium text-foreground-muted uppercase tracking-wider">Project Context</h3>
          <p className="text-[10px] text-foreground-subtle">
            Connect projects so the AI researches within your actual stack and architecture.
          </p>
          <div className="space-y-1.5">
            {availableProjects.map((proj) => {
              const isConnected = connectedProjects.includes(proj.id)
              return (
                <button
                  key={proj.id}
                  onClick={() => toggleProject(proj.id)}
                  className={cn(
                    'w-full flex items-center gap-3 rounded-md px-3 py-2 text-left transition-colors',
                    isConnected
                      ? 'bg-brand/10 border border-brand/30'
                      : 'bg-background-secondary border border-border/50 hover:border-border',
                  )}
                >
                  <FolderGit2 className={cn('w-4 h-4 shrink-0', isConnected ? 'text-brand' : 'text-foreground-muted')} />
                  <div className="flex-1 min-w-0">
                    <div className="text-xs font-medium text-foreground">{proj.name}</div>
                    <div className="text-[9px] text-foreground-subtle truncate">{proj.stack}</div>
                  </div>
                  <div className={cn(
                    'w-4 h-4 rounded-full border flex items-center justify-center shrink-0',
                    isConnected
                      ? 'border-brand bg-brand'
                      : 'border-foreground-subtle',
                  )}>
                    {isConnected && <Check className="w-2.5 h-2.5 text-white" />}
                  </div>
                </button>
              )
            })}
          </div>
          {connectedProjects.length > 0 && (
            <p className="text-[9px] text-foreground-subtle">
              The AI will read .context.md files and code index from {connectedProjects.length} connected project{connectedProjects.length > 1 ? 's' : ''}.
            </p>
          )}
        </section>

        {/* Research Parameters */}
        <section className="space-y-3">
          <h3 className="text-xs font-medium text-foreground-muted uppercase tracking-wider">Research Parameters</h3>

          {/* Intensity */}
          <div className="space-y-2">
            <label className="text-[11px] text-foreground-muted">Search intensity</label>
            <div className="space-y-1.5">
              {INTENSITIES.map((i) => (
                <button
                  key={i.value}
                  onClick={() => setIntensity(i.value as ResearchIntensity)}
                  className={cn(
                    'w-full flex items-start gap-3 rounded-md px-3 py-2 text-left transition-colors',
                    intensity === i.value
                      ? 'bg-brand/10 border border-brand/30'
                      : 'bg-background-secondary border border-border/50 hover:border-border',
                  )}
                >
                  <div className={cn(
                    'mt-0.5 w-3 h-3 rounded-full border-2 shrink-0',
                    intensity === i.value ? 'border-brand bg-brand' : 'border-foreground-subtle',
                  )} />
                  <div>
                    <div className="text-xs font-medium text-foreground">{i.label}</div>
                    <div className="text-[10px] text-foreground-muted">{i.desc}</div>
                  </div>
                </button>
              ))}
            </div>
          </div>

          {/* Max hexagons per phase */}
          <div className="space-y-1.5">
            <label className="text-[11px] text-foreground-muted">Max research points per phase</label>
            <input
              type="number"
              value={maxHexagons}
              onChange={(e) => setMaxHexagons(Number(e.target.value))}
              min={10}
              max={200}
              className="w-32 rounded-md border border-border/50 bg-background-secondary px-3 py-1.5 text-sm text-foreground outline-none focus:border-brand transition-colors"
            />
            <p className="text-[10px] text-foreground-subtle">Limits how many hexagons the AI can create per phase (default: 100)</p>
          </div>

          {/* Auto advance */}
          <div className="flex items-center justify-between rounded-md bg-background-secondary border border-border/50 px-3 py-2">
            <div>
              <div className="text-xs font-medium text-foreground">Auto-advance phases</div>
              <div className="text-[10px] text-foreground-muted">AI automatically moves hexagons to the next phase when ready</div>
            </div>
            <button
              onClick={() => setAutoAdvance(!autoAdvance)}
              className={cn(
                'relative w-8 h-4.5 rounded-full transition-colors shrink-0',
                autoAdvance ? 'bg-brand' : 'bg-foreground-subtle/30',
              )}
            >
              <div className={cn(
                'absolute top-0.5 w-3.5 h-3.5 rounded-full bg-white transition-transform',
                autoAdvance ? 'translate-x-4' : 'translate-x-0.5',
              )} />
            </button>
          </div>
        </section>

        {/* Spacer for sticky save button */}
        {isDirty && <div className="h-12" />}
      </div>

      {/* Sticky Save button */}
      {isDirty && (
        <div className="sticky bottom-0 bg-background/90 backdrop-blur-sm border-t border-border/50 px-6 py-3">
          <button
            onClick={handleSave}
            disabled={saving}
            className="flex items-center gap-1.5 px-4 py-1.5 rounded-md text-xs font-medium bg-brand text-white hover:bg-brand/90 disabled:opacity-50 transition-colors"
          >
            <Save className="w-3.5 h-3.5" />
            {saving ? 'Saving…' : 'Save changes'}
          </button>
        </div>
      )}
    </div>
  )
}
