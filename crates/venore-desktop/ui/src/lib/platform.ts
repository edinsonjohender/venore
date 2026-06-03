import { platform } from '@tauri-apps/plugin-os'

export const isMacOS = platform() === 'macos'
