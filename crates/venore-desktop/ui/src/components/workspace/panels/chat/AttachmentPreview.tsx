// =============================================================================
// AttachmentPreview - Horizontal row of attachment thumbnails/chips
// =============================================================================

import { X, FileText, FileCode, Image, File } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import type { Attachment } from '@/hooks/useAttachments'
import { cn } from '@/lib/utils'

interface AttachmentPreviewProps {
  attachments: Attachment[]
  onRemove: (id: string) => void
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes}B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)}KB`
  return `${(bytes / (1024 * 1024)).toFixed(1)}MB`
}

function getFileIcon(mimeType: string) {
  if (mimeType.startsWith('image/')) return Image
  if (mimeType.includes('javascript') || mimeType.includes('typescript') || mimeType.includes('json') || mimeType.includes('rust') || mimeType.includes('python')) return FileCode
  if (mimeType.startsWith('text/')) return FileText
  return File
}

export function AttachmentPreview({ attachments, onRemove }: AttachmentPreviewProps) {
  const { t } = useTranslation('chat')

  if (attachments.length === 0) return null

  return (
    <div className="flex flex-wrap gap-2 px-3 pt-2.5 pb-1">
      {attachments.map((att) =>
        att.isImage && att.thumbnailUrl ? (
          <ImageThumbnail
            key={att.id}
            attachment={att}
            onRemove={() => onRemove(att.id)}
            removeLabel={t('input.removeAttachment', 'Remove')}
          />
        ) : (
          <FileChip
            key={att.id}
            attachment={att}
            onRemove={() => onRemove(att.id)}
            removeLabel={t('input.removeAttachment', 'Remove')}
          />
        ),
      )}
    </div>
  )
}

function ImageThumbnail({
  attachment,
  onRemove,
  removeLabel,
}: {
  attachment: Attachment
  onRemove: () => void
  removeLabel: string
}) {
  return (
    <div className="group relative animate-in fade-in-0 zoom-in-95 duration-200">
      <img
        src={attachment.thumbnailUrl!}
        alt={attachment.name}
        className="w-14 h-14 rounded-lg object-cover border border-border"
      />
      <button
        type="button"
        onClick={onRemove}
        className="absolute -top-1.5 -right-1.5 h-5 w-5 flex items-center justify-center rounded-full bg-background-secondary border border-border text-foreground-muted opacity-0 group-hover:opacity-100 transition-opacity hover:text-foreground hover:bg-background-tertiary"
        title={removeLabel}
      >
        <X className="w-3 h-3" />
      </button>
    </div>
  )
}

function FileChip({
  attachment,
  onRemove,
  removeLabel,
}: {
  attachment: Attachment
  onRemove: () => void
  removeLabel: string
}) {
  const Icon = getFileIcon(attachment.mimeType)

  return (
    <div
      className={cn(
        'group flex items-center gap-1.5 px-2 py-1.5 rounded-lg border border-border bg-background-secondary',
        'animate-in fade-in-0 zoom-in-95 duration-200',
      )}
    >
      <Icon className="w-3.5 h-3.5 text-foreground-muted shrink-0" />
      <span className="text-xs text-foreground truncate max-w-[120px]">{attachment.name}</span>
      <span className="text-[10px] text-foreground-muted shrink-0">{formatSize(attachment.size)}</span>
      <button
        type="button"
        onClick={onRemove}
        className="ml-0.5 h-4 w-4 flex items-center justify-center rounded text-foreground-muted opacity-0 group-hover:opacity-100 transition-opacity hover:text-foreground"
        title={removeLabel}
      >
        <X className="w-3 h-3" />
      </button>
    </div>
  )
}
