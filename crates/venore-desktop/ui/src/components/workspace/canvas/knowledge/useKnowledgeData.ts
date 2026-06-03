// =============================================================================
// useKnowledgeData — Hook to fetch & cache knowledge data from backend
// =============================================================================
// Replaces MOCK_FEATURES with real backend data. Listens to Tauri events
// for real-time updates when the AI modifies hexagons/evidence.

import { useState, useEffect, useCallback } from 'react'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { tauriApi } from '@/lib/tauri'
import type { FeatureResponse, HexagonResponse, EvidenceResponse } from '@/lib/tauri'
import type { Feature, Hexagon, Evidence, HexPhase, ResearchIntensity } from './mock-data'

// -----------------------------------------------------------------------------
// Converters: backend responses → UI types
// -----------------------------------------------------------------------------

export function featureFromResponse(r: FeatureResponse, hexagons: Hexagon[], evidence: Evidence[]): Feature {
  let tags: string[] = []
  try { tags = JSON.parse(r.tags || '[]') } catch { /* empty */ }
  return {
    id: r.id,
    name: r.name,
    description: r.description,
    objective: (r.objective || 'explore') as Feature['objective'],
    status: (r.status || 'active') as Feature['status'],
    priority: (r.priority || 'medium') as Feature['priority'],
    intensity: (r.intensity || 'moderate') as ResearchIntensity,
    maxHexagonsPerPhase: r.maxHexagonsPerPhase ?? 100,
    autoAdvance: r.autoAdvance ?? true,
    tags,
    hexagons,
    evidence,
  }
}

export function hexagonFromResponse(r: HexagonResponse): Hexagon {
  let blockedBy: string[] = []
  try { blockedBy = JSON.parse(r.blockedBy || '[]') } catch { /* empty */ }
  return {
    id: r.id,
    featureId: r.featureId,
    parentId: null,
    title: r.title,
    description: r.description,
    phase: (r.phase || 'discover') as HexPhase,
    percentage: r.percentage,
    isDeadEnd: r.isDeadEnd,
    confidence: (r.confidence || 'low') as Hexagon['confidence'],
    risk: (r.risk || 'unknown') as Hexagon['risk'],
    notes: r.notesUser,
    blockedBy,
  }
}

export function evidenceFromResponse(r: EvidenceResponse): Evidence {
  return {
    id: r.id,
    hexagonId: r.hexagonId,
    type: r.sourceType === 'web' ? 'link' : r.sourceType === 'document' ? 'file' : 'note',
    title: r.content.slice(0, 80) + (r.content.length > 80 ? '…' : ''),
    url: r.sourceUrl || undefined,
    content: r.content,
  }
}

// -----------------------------------------------------------------------------
// Hook: single feature with hexagons + evidence
// -----------------------------------------------------------------------------

export function useKnowledgeFeature(featureId: string) {
  const [feature, setFeature] = useState<Feature | null>(null)
  const [loading, setLoading] = useState(true)

  const reload = useCallback(async () => {
    try {
      const fRes = await tauriApi.getKnowledgeFeature(featureId)
      if (!fRes) { setFeature(null); return }

      const hexRes = await tauriApi.listKnowledgeHexagons(featureId)
      const hexagons = (hexRes ?? []).map(hexagonFromResponse)

      // Load evidence for all hexagons
      const allEvidence: Evidence[] = []
      for (const hex of hexagons) {
        const evRes = await tauriApi.listKnowledgeEvidence(hex.id)
        allEvidence.push(...(evRes ?? []).map(evidenceFromResponse))
      }

      setFeature(featureFromResponse(fRes, hexagons, allEvidence))
    } catch (err) {
      console.error('Failed to load knowledge feature:', err)
      setFeature(null)
    } finally {
      setLoading(false)
    }
  }, [featureId])

  // Initial load
  useEffect(() => {
    setLoading(true)
    reload()
  }, [reload])

  // Listen for real-time updates
  useEffect(() => {
    let unlisten: UnlistenFn | null = null
    listen<{ featureId: string; toolName: string }>('knowledge-hexagons-changed', (event) => {
      if (event.payload.featureId === featureId) {
        reload()
      }
    }).then((fn) => { unlisten = fn })
    return () => { unlisten?.() }
  }, [featureId, reload])

  return { feature, loading, reload }
}

// -----------------------------------------------------------------------------
// Hook: feature list for sidebar panel
// -----------------------------------------------------------------------------

export function useKnowledgeFeatures(projectId: string | null) {
  const [features, setFeatures] = useState<Feature[]>([])
  const [loading, setLoading] = useState(true)

  const reload = useCallback(async () => {
    if (!projectId) { setFeatures([]); setLoading(false); return }
    try {
      const fList = await tauriApi.listKnowledgeFeatures(projectId)
      if (!fList) { setFeatures([]); return }

      // For sidebar, load hexagons but skip evidence (too expensive)
      const results: Feature[] = []
      for (const fRes of fList) {
        const hexRes = await tauriApi.listKnowledgeHexagons(fRes.id)
        const hexagons = (hexRes ?? []).map(hexagonFromResponse)
        results.push(featureFromResponse(fRes, hexagons, []))
      }
      setFeatures(results)
    } catch (err) {
      console.error('Failed to load knowledge features:', err)
      setFeatures([])
    } finally {
      setLoading(false)
    }
  }, [projectId])

  useEffect(() => {
    setLoading(true)
    reload()
  }, [reload])

  // Listen for changes
  useEffect(() => {
    let unlisten: UnlistenFn | null = null
    listen('knowledge-hexagons-changed', () => { reload() }).then((fn) => { unlisten = fn })
    return () => { unlisten?.() }
  }, [reload])

  return { features, loading, reload }
}
