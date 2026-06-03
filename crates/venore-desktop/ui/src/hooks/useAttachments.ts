// =============================================================================
// useAttachments - Manage file attachments for chat input
// =============================================================================
// Ephemeral per-message lifecycle: add from file dialog / drag-drop / paste,
// preview thumbnails, convert to base64 for sending, clear after send.

import { useState, useCallback, useRef } from 'react'
import { tauriApi } from '@/lib/tauri'
import type { AttachmentInput, FileAttachmentData } from '@/lib/tauri'

const MAX_ATTACHMENTS = 10
const MAX_FILE_SIZE = 20 * 1024 * 1024 // 20 MB

export interface Attachment {
  id: string
  name: string
  mimeType: string
  size: number
  isImage: boolean
  dataBase64: string
  thumbnailUrl: string | null
}

export interface UseAttachmentsResult {
  attachments: Attachment[]
  addFromPaths: (paths: string[]) => Promise<void>
  addFromClipboard: (items: DataTransferItemList) => Promise<void>
  remove: (id: string) => void
  clear: () => void
  toInputArray: () => AttachmentInput[]
}

function createThumbnailUrl(mimeType: string, base64: string): string | null {
  if (!mimeType.startsWith('image/')) return null
  return `data:${mimeType};base64,${base64}`
}

export function useAttachments(): UseAttachmentsResult {
  const [attachments, setAttachments] = useState<Attachment[]>([])
  const pendingRef = useRef(false)

  const addFromPaths = useCallback(async (paths: string[]) => {
    if (pendingRef.current) return
    pendingRef.current = true

    try {
      const remaining = MAX_ATTACHMENTS - attachments.length
      const toProcess = paths.slice(0, remaining)

      const results: Attachment[] = []
      for (const path of toProcess) {
        try {
          const data: FileAttachmentData = await tauriApi.readFileForAttachment(path)

          // Deduplicate by name + size
          const isDuplicate = attachments.some(
            (a) => a.name === data.name && a.size === data.size,
          )
          if (isDuplicate) continue

          results.push({
            id: crypto.randomUUID(),
            name: data.name,
            mimeType: data.mime_type,
            size: data.size,
            isImage: data.is_image,
            dataBase64: data.data_base64,
            thumbnailUrl: createThumbnailUrl(data.mime_type, data.data_base64),
          })
        } catch (err) {
          console.warn('Failed to read attachment:', path, err)
        }
      }

      if (results.length > 0) {
        setAttachments((prev) => [...prev, ...results].slice(0, MAX_ATTACHMENTS))
      }
    } finally {
      pendingRef.current = false
    }
  }, [attachments])

  const addFromClipboard = useCallback(async (items: DataTransferItemList) => {
    if (attachments.length >= MAX_ATTACHMENTS) return

    for (let i = 0; i < items.length; i++) {
      const item = items[i]
      if (!item.type.startsWith('image/')) continue

      const file = item.getAsFile()
      if (!file) continue
      if (file.size > MAX_FILE_SIZE) continue

      try {
        const base64 = await fileToBase64(file)
        const att: Attachment = {
          id: crypto.randomUUID(),
          name: file.name || `pasted-image-${Date.now()}.png`,
          mimeType: file.type,
          size: file.size,
          isImage: true,
          dataBase64: base64,
          thumbnailUrl: createThumbnailUrl(file.type, base64),
        }
        setAttachments((prev) => [...prev, att].slice(0, MAX_ATTACHMENTS))
      } catch {
        console.warn('Failed to read pasted image')
      }
      break // Only one image per paste
    }
  }, [attachments.length])

  const remove = useCallback((id: string) => {
    setAttachments((prev) => prev.filter((a) => a.id !== id))
  }, [])

  const clear = useCallback(() => {
    setAttachments([])
  }, [])

  const toInputArray = useCallback((): AttachmentInput[] => {
    return attachments.map((a) => ({
      name: a.name,
      mime_type: a.mimeType,
      data_base64: a.dataBase64,
    }))
  }, [attachments])

  return { attachments, addFromPaths, addFromClipboard, remove, clear, toInputArray }
}

/** Convert a File to base64 string (without the data: prefix) */
function fileToBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => {
      const result = reader.result as string
      // Strip the "data:image/png;base64," prefix
      const base64 = result.split(',')[1] ?? ''
      resolve(base64)
    }
    reader.onerror = reject
    reader.readAsDataURL(file)
  })
}
