import { useEffect } from 'react'
import { listen } from '@tauri-apps/api/event'
import { toast } from 'sonner'
import { getErrorInfo } from '@/lib/error-messages'

interface AppNotification {
  level: 'error' | 'warning' | 'info' | 'success'
  title: string
  description?: string
  code?: string
}

export function useAppNotifications() {
  useEffect(() => {
    let unlisten: (() => void) | undefined

    listen<AppNotification>('app:notification', (event) => {
      const { level, title, description, code } = event.payload

      // i18n lookup if error code is provided
      let msg = title
      let desc = description
      if (code) {
        const info = getErrorInfo(code, title)
        msg = info.message
        if (info.suggestion) desc = info.suggestion
      }

      toast[level](msg, { description: desc })
    }).then((fn) => { unlisten = fn })

    return () => { unlisten?.() }
  }, [])
}
