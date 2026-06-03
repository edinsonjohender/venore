// =============================================================================
// AuthButton - Optional cloud auth widget for ActivityBar
// =============================================================================
// Signed out: User icon, click opens AuthModal.
// Signed in: Avatar (initials fallback), click for dropdown with sign out.

import { useState, useCallback } from 'react'
import { User, LogOut } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { cn } from '@/lib/utils'
import { useAuthStore } from '@/stores/authStore'
import { AuthModal } from '@/components/cloud/AuthModal'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'

// -----------------------------------------------------------------------------
// Avatar with initials fallback
// -----------------------------------------------------------------------------

function UserAvatar({ displayName, avatarUrl }: { displayName: string; avatarUrl: string | null }) {
  const initials = displayName
    .split(' ')
    .map((n) => n[0])
    .join('')
    .slice(0, 2)
    .toUpperCase()

  if (avatarUrl) {
    return (
      <img
        src={avatarUrl}
        alt={displayName}
        className="w-6 h-6 rounded-full object-cover"
        draggable={false}
      />
    )
  }

  return (
    <div className="w-6 h-6 rounded-full bg-brand/20 text-brand flex items-center justify-center text-[10px] font-medium">
      {initials || 'U'}
    </div>
  )
}

// -----------------------------------------------------------------------------
// Main Component
// -----------------------------------------------------------------------------

export function AuthButton() {
  const { t } = useTranslation('menu')
  const { authenticated, user, loading, signOut } = useAuthStore()
  const [modalOpen, setModalOpen] = useState(false)

  const handleSignOut = useCallback(() => {
    if (!loading) signOut()
  }, [loading, signOut])

  // Not signed in: show User icon + modal
  if (!authenticated || !user) {
    return (
      <>
        <button
          className={cn(
            'flex items-center justify-center w-full h-10 transition-colors',
            'text-foreground-subtle hover:text-foreground',
          )}
          title={t('signIn')}
          onClick={() => setModalOpen(true)}
        >
          <User className="w-[18px] h-[18px]" />
        </button>
        <AuthModal open={modalOpen} onOpenChange={setModalOpen} />
      </>
    )
  }

  // Signed in: show avatar with dropdown
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button
          className={cn(
            'flex items-center justify-center w-full h-10 transition-colors',
            'text-foreground-subtle hover:text-foreground',
          )}
          title={t('account')}
        >
          <UserAvatar displayName={user.displayName} avatarUrl={user.avatarUrl} />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent
        side="right"
        align="end"
        sideOffset={4}
        className="min-w-[180px] bg-background-tertiary border-border rounded-md p-1 shadow-xl shadow-black/50"
      >
        {/* User info */}
        <div className="px-3 py-2">
          <p className="text-xs font-medium text-foreground truncate">
            {user.displayName}
          </p>
          <p className="text-[10px] text-foreground-muted truncate">
            {user.email}
          </p>
        </div>
        <DropdownMenuSeparator className="-mx-1 my-1 h-px bg-border" />
        <DropdownMenuItem
          className="text-xs px-3 py-1.5 rounded-sm text-foreground-muted cursor-default select-none outline-none focus:bg-background-secondary focus:text-foreground"
          onSelect={handleSignOut}
        >
          <LogOut className="w-3.5 h-3.5 mr-2" />
          {t('signOut')}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
