/**
 * Adapters to convert between backend (Tauri) types and frontend (Wizard) types
 *
 * Backend uses snake_case (Rust convention)
 * Frontend uses camelCase (TypeScript convention)
 */

import type {
  DetectedModule as BackendModule,
  ProjectMetrics as BackendMetrics,
} from '@/lib/tauri'

import type {
  ModuleInfo,
} from './types'

// =============================================================================
// Module Adapters
// =============================================================================

export function adaptModuleFromBackend(backendModule: BackendModule): ModuleInfo {
  return {
    id: backendModule.id,
    name: backendModule.name,
    path: backendModule.path,
    fileCount: backendModule.file_count,
    moduleType: 'component', // Default, could be inferred from path
    confidence: backendModule.confidence,
    hasExistingContext: backendModule.has_existing_context,
    entryPoint: backendModule.entry_point || undefined,
    description: backendModule.description,
  }
}

export function adaptMetricsFromBackend(backendMetrics: BackendMetrics) {
  return {
    totalFiles: backendMetrics.total_files,
    detectedModules: backendMetrics.total_modules,
  }
}
