// =============================================================================
// ChatTaskList - Inline task checklist for task_create/update tools
// =============================================================================
// Shows a checklist of tasks with status indicators.

import { Loader2, Check, Circle } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { cn } from '@/lib/utils'
import type { TaskItemPayload } from '@/stores/chatStore'

interface ChatTaskListProps {
  tasks: TaskItemPayload[]
  /** When true, renders without outer border/bg (for use inside overlay panels) */
  embedded?: boolean
}

function StatusIcon({ status }: { status: string }) {
  switch (status) {
    case 'completed':
      return <Check className="w-3.5 h-3.5 text-emerald-400/70" />
    case 'in_progress':
      return <Loader2 className="w-3.5 h-3.5 text-foreground-muted animate-spin" />
    default:
      return <Circle className="w-3.5 h-3.5 text-foreground-muted" />
  }
}

export function ChatTaskList({ tasks, embedded }: ChatTaskListProps) {
  const { t } = useTranslation('chat')

  if (tasks.length === 0) return null

  const completed = tasks.filter((t) => t.status === 'completed').length
  const total = tasks.length

  return (
    <div className={cn(
      'overflow-hidden',
      embedded
        ? 'rounded'
        : 'my-2 rounded-lg border border-border bg-background-secondary/50',
    )}>
      {/* Header */}
      <div className={cn(
        'flex items-center gap-2',
        embedded ? 'px-2 py-1.5 border-b border-border/50' : 'px-3 py-2 border-b border-border',
      )}>
        <span className="text-[10px] font-mono text-foreground-subtle uppercase tracking-wider">
          {t('taskList.tasks')}
        </span>
        <span className="flex-1" />
        <span className="text-[10px] font-mono text-foreground-muted">
          {completed}/{total}
        </span>
      </div>

      {/* Task list */}
      <div className="px-3 py-1.5">
        {tasks.map((task) => (
          <div
            key={task.id}
            className={cn(
              'flex items-start gap-2 py-1.5',
              task.status === 'completed' && 'opacity-60',
            )}
          >
            <div className="mt-0.5 shrink-0">
              <StatusIcon status={task.status} />
            </div>
            <span
              className={cn(
                'text-xs text-foreground',
                task.status === 'completed' && 'line-through',
              )}
            >
              {task.subject}
            </span>
          </div>
        ))}
      </div>

      {/* Progress bar */}
      <div className="px-3 pb-2">
        <div className="h-1 bg-background-tertiary rounded-full overflow-hidden">
          <div
            className="h-full bg-brand transition-all duration-300"
            style={{ width: `${total > 0 ? (completed / total) * 100 : 0}%` }}
          />
        </div>
      </div>
    </div>
  )
}
