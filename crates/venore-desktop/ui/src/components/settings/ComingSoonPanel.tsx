// =============================================================================
// ComingSoonPanel — placeholder for unbuilt Settings sections
// =============================================================================
// Filler component used by settings.config.ts so the sidebar entry can list
// a section before its real UI exists. Keeps the user oriented: they see the
// label, the icon, and a clear "not built yet" message instead of a blank
// pane.

import { Sparkles } from 'lucide-react'

export function ComingSoonPanel() {
  return (
    <div className="h-full flex flex-col items-center justify-center text-center px-6">
      <div className="p-3 rounded-full bg-background-tertiary text-foreground-muted mb-4">
        <Sparkles size={28} />
      </div>
      <h4 className="text-base font-medium text-foreground">Coming soon</h4>
      <p className="text-sm text-foreground-muted mt-1 max-w-sm">
        This section isn&apos;t available yet. We&apos;re shipping it in a
        future release — the affordance is here so the navigation stays stable.
      </p>
    </div>
  )
}

export default ComingSoonPanel
