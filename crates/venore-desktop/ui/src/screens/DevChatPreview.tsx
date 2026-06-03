// =============================================================================
// DevChatPreview - Visual test page for all chat components
// =============================================================================
// Access: add #dev-chat to the URL or press Ctrl+Shift+D in the app.
// Renders every chat component with mock data for visual QA.

import { useState } from 'react'
import { ChatMessage } from '@/components/workspace/panels/chat/ChatMessage'
import { ChatToolCall } from '@/components/workspace/panels/chat/ChatToolCall'
import { ChatSubAgent } from '@/components/workspace/panels/chat/ChatSubAgent'
import { ChatTaskList } from '@/components/workspace/panels/chat/ChatTaskList'
import { FloatingOverlay } from '@/components/workspace/panels/chat/overlay/FloatingOverlay'
import { FloatingOverlayHeader } from '@/components/workspace/panels/chat/overlay/FloatingOverlayHeader'
import { Activity, ShieldAlert, MessageCircleQuestion, ClipboardList } from 'lucide-react'
import type { ChatMessage as ChatMessageType, ToolCallInfo, SubAgentPayload, TaskItemPayload } from '@/stores/chatStore'

// -----------------------------------------------------------------------------
// Mock data
// -----------------------------------------------------------------------------

const MOCK_MESSAGES: ChatMessageType[] = [
  { id: 'u1', role: 'user', content: 'Analyze the project and generate the context files.', timestamp: Date.now() - 60000 },
  { id: 'sys1', role: 'system', content: 'Session started', timestamp: Date.now() - 55000 },
  { id: 'a1', role: 'assistant', content: 'I will analyze the project structure. First I will review the configuration files and then scan the main modules.\n\n```rust\nfn main() {\n    println!("Hello, world!");\n}\n```\n\nThis is an inline code example.', timestamp: Date.now() - 50000 },
  { id: 'a2', role: 'assistant', content: '', timestamp: Date.now(), isStreaming: true },
]

const MOCK_TOOL_CALLS: { toolCall: ToolCallInfo; messageId: string }[] = [
  { toolCall: { id: 'tc1', name: 'read_file', arguments: { file_path: 'src/main.rs' }, status: 'completed', result: 'fn main() {\n    println!("Hello");\n}' }, messageId: 'a1' },
  { toolCall: { id: 'tc2', name: 'run_terminal_command', arguments: { command: 'cargo build --release' }, status: 'running' }, messageId: 'a1' },
  { toolCall: { id: 'tc3', name: 'write_file', arguments: { file_path: 'src/lib.rs', content: '// new file' }, status: 'error', result: 'Permission denied' }, messageId: 'a1' },
  { toolCall: { id: 'tc4', name: 'web_search', arguments: { query: 'rust async patterns' }, status: 'denied' }, messageId: 'a1' },
  { toolCall: { id: 'tc5', name: 'edit_file', arguments: { file_path: 'Cargo.toml', old_string: 'v0.1', new_string: 'v0.2' }, status: 'completed', commitHash: 'abc123' }, messageId: 'a1' },
]

const MOCK_SUB_AGENTS: SubAgentPayload[] = [
  { agent_id: 'sa1', agent_type: 'research', task: 'Research how routes (routing) work in the project. Analyze the patterns.', status: 'completed', result: 'Found 3 route files in src/routes/' },
  { agent_id: 'sa2', agent_type: 'research', task: 'Research how permissions are managed in desktop applications.', status: 'completed', result: null },
  { agent_id: 'sa3', agent_type: 'research', task: 'Research the concept of "special events" or any similar mechanism.', status: 'started', result: null },
  { agent_id: 'sa4', agent_type: 'code', task: 'Start the application', status: 'started', result: null },
]

const MOCK_TASKS: TaskItemPayload[] = [
  { id: 't1', subject: 'Analyze project structure', status: 'completed', description: '' },
  { id: 't2', subject: 'Detect modules and dependencies', status: 'completed', description: '' },
  { id: 't3', subject: 'Generate .context.md files', status: 'in_progress', description: '' },
  { id: 't4', subject: 'Validate generated contexts', status: 'pending', description: '' },
]

// -----------------------------------------------------------------------------
// Section wrapper
// -----------------------------------------------------------------------------

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="mb-8">
      <h2 className="text-xs font-mono font-semibold text-foreground-muted uppercase tracking-widest mb-3 border-b border-border pb-2">
        {title}
      </h2>
      {children}
    </div>
  )
}

function SubSection({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="mb-4">
      <h3 className="text-[10px] font-mono text-foreground-subtle uppercase tracking-wider mb-2">{title}</h3>
      {children}
    </div>
  )
}

// -----------------------------------------------------------------------------
// Mock overlay components (with real collapse state)
// -----------------------------------------------------------------------------

function MockExecutionOverlay({ onClose }: { onClose: () => void }) {
  const [collapsed, setCollapsed] = useState(false)
  return (
    <FloatingOverlay accentColor="emerald" onDismiss={onClose}>
      <FloatingOverlayHeader
        icon={Activity}
        title="Execution"
        accentColor="emerald"
        badge="2 running"
        isCollapsed={collapsed}
        onToggleCollapse={() => setCollapsed((c) => !c)}
        onClose={onClose}
      />
      {!collapsed && (
        <div className="px-2 py-1.5">
          {MOCK_TOOL_CALLS.slice(0, 2).map(({ toolCall, messageId }) => (
            <ChatToolCall key={toolCall.id} toolCall={toolCall} messageId={messageId} embedded />
          ))}
          {MOCK_SUB_AGENTS.slice(0, 2).map((sa) => (
            <ChatSubAgent key={sa.agent_id} payload={sa} embedded />
          ))}
        </div>
      )}
    </FloatingOverlay>
  )
}

function MockToolConfirmOverlay({ onClose }: { onClose: () => void }) {
  const [collapsed, setCollapsed] = useState(false)
  return (
    <FloatingOverlay accentColor="amber" onDismiss={onClose}>
      <FloatingOverlayHeader
        icon={ShieldAlert}
        title="AI wants to use a tool"
        accentColor="amber"
        badge="Terminal Command"
        isCollapsed={collapsed}
        onToggleCollapse={() => setCollapsed((c) => !c)}
        onClose={onClose}
      />
      {!collapsed && (
        <>
          <div className="px-3 py-2.5">
            <div className="rounded bg-background-tertiary/80 px-2.5 py-1.5">
              <code className="text-xs font-mono text-foreground">cargo build --release</code>
            </div>
          </div>
          <div className="flex items-center gap-2 px-3 pb-3">
            <button className="px-3 py-1 text-xs font-medium rounded bg-brand text-background">Allow once</button>
            <button className="px-3 py-1 text-xs font-medium rounded border border-brand/40 text-brand">Allow for session</button>
            <button className="px-3 py-1 text-xs font-medium rounded border border-border text-foreground-muted">Deny</button>
          </div>
        </>
      )}
    </FloatingOverlay>
  )
}

function MockAskUserOverlay({ onClose }: { onClose: () => void }) {
  const [collapsed, setCollapsed] = useState(false)
  return (
    <FloatingOverlay accentColor="brand" onDismiss={onClose}>
      <FloatingOverlayHeader
        icon={MessageCircleQuestion}
        title="Agent needs your input"
        accentColor="brand"
        isCollapsed={collapsed}
        onToggleCollapse={() => setCollapsed((c) => !c)}
        onClose={onClose}
      />
      {!collapsed && (
        <>
          <div className="px-3 py-2.5">
            <p className="text-sm text-foreground">Which testing framework should I use for this project?</p>
          </div>
          <div className="px-3 pb-2 flex flex-wrap gap-1.5">
            {['Jest', 'Vitest', 'Playwright'].map((opt) => (
              <button key={opt} className="px-3 py-1.5 text-xs font-medium rounded-lg border border-border bg-background-tertiary text-foreground">
                {opt}
              </button>
            ))}
          </div>
          <div className="px-3 pb-2.5 flex gap-2">
            <input placeholder="Type a response..." className="flex-1 h-8 px-2.5 text-xs bg-background-tertiary border border-border rounded-lg text-foreground placeholder:text-foreground-subtle/50 outline-none" />
          </div>
        </>
      )}
    </FloatingOverlay>
  )
}

function MockPlanViewOverlay({ onClose }: { onClose: () => void }) {
  const [collapsed, setCollapsed] = useState(false)
  return (
    <FloatingOverlay accentColor="blue" onDismiss={onClose}>
      <FloatingOverlayHeader
        icon={ClipboardList}
        title="Plan for approval"
        accentColor="blue"
        isCollapsed={collapsed}
        onToggleCollapse={() => setCollapsed((c) => !c)}
        onClose={onClose}
      />
      {!collapsed && (
        <>
          <div className="px-3 py-2.5">
            <p className="text-sm text-foreground">Refactor the authentication module to use JWT tokens.</p>
          </div>
          <div className="px-3 pb-2">
            <ol className="space-y-1">
              {['Create JWT utility module', 'Update auth middleware', 'Migrate session store', 'Add refresh token flow'].map((step, i) => (
                <li key={i} className="flex items-start gap-2 text-xs text-foreground-muted">
                  <span className="font-mono text-foreground-subtle shrink-0">{i + 1}.</span>
                  <span>{step}</span>
                </li>
              ))}
            </ol>
          </div>
          <div className="flex items-center gap-2 px-3 py-2.5 border-t border-border">
            <button className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded border border-border bg-background-tertiary/50 text-foreground-muted hover:bg-emerald-500/10 hover:text-emerald-400/90 hover:border-emerald-500/30 transition-colors">Approve</button>
            <button className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded border border-border bg-background-tertiary/50 text-foreground-muted hover:bg-red-500/10 hover:text-red-400/90 hover:border-red-500/30 transition-colors">Reject</button>
          </div>
        </>
      )}
    </FloatingOverlay>
  )
}

// -----------------------------------------------------------------------------
// Main preview
// -----------------------------------------------------------------------------

export function DevChatPreview() {
  const [activeOverlay, setActiveOverlay] = useState<string | null>(null)

  return (
    <div className="h-screen w-screen bg-background overflow-y-auto">
      <div className="max-w-3xl mx-auto px-6 py-8">
        {/* Title */}
        <div className="mb-8">
          <h1 className="text-lg font-semibold text-foreground">Chat Components Preview</h1>
          <p className="text-xs text-foreground-muted mt-1">Visual QA for all chat UI components. Press Ctrl+Shift+D to toggle.</p>
        </div>

        {/* ── Messages ─────────────────────────────────────────────────── */}
        <Section title="Messages">
          <div className="flex flex-col gap-3 p-3 bg-background-secondary rounded-lg border border-border">
            {MOCK_MESSAGES.map((msg) => (
              <ChatMessage key={msg.id} message={msg} />
            ))}
          </div>
        </Section>

        {/* ── Tool Calls — Standard vs Embedded ─────────────────────── */}
        <Section title="Tool Calls">
          <div className="grid grid-cols-2 gap-4">
            <SubSection title="Standard">
              {MOCK_TOOL_CALLS.map(({ toolCall, messageId }) => (
                <ChatToolCall key={toolCall.id} toolCall={toolCall} messageId={messageId} />
              ))}
            </SubSection>
            <SubSection title="Embedded (overlay mode)">
              {MOCK_TOOL_CALLS.map(({ toolCall, messageId }) => (
                <ChatToolCall key={toolCall.id} toolCall={toolCall} messageId={messageId} embedded />
              ))}
            </SubSection>
          </div>
        </Section>

        {/* ── Sub-Agents — Standard vs Embedded ──────────────────────── */}
        <Section title="Sub-Agents">
          <div className="grid grid-cols-2 gap-4">
            <SubSection title="Standard">
              {MOCK_SUB_AGENTS.map((sa) => (
                <ChatSubAgent key={sa.agent_id} payload={sa} />
              ))}
            </SubSection>
            <SubSection title="Embedded (overlay mode)">
              {MOCK_SUB_AGENTS.map((sa) => (
                <ChatSubAgent key={sa.agent_id} payload={sa} embedded />
              ))}
            </SubSection>
          </div>
        </Section>

        {/* ── Task List — Standard vs Embedded ───────────────────────── */}
        <Section title="Task List">
          <div className="grid grid-cols-2 gap-4">
            <SubSection title="Standard">
              <ChatTaskList tasks={MOCK_TASKS} />
            </SubSection>
            <SubSection title="Embedded (overlay mode)">
              <ChatTaskList tasks={MOCK_TASKS} embedded />
            </SubSection>
          </div>
        </Section>

        {/* ── Floating Overlays ──────────────────────────────────────── */}
        <Section title="Floating Overlays (click to preview)">
          <div className="flex gap-2 mb-4">
            {['execution', 'toolConfirm', 'askUser', 'planView'].map((id) => (
              <button
                key={id}
                type="button"
                onClick={() => setActiveOverlay(activeOverlay === id ? null : id)}
                className={`px-3 py-1.5 text-xs font-medium rounded-lg border transition-colors ${
                  activeOverlay === id
                    ? 'bg-brand/20 border-brand/40 text-brand'
                    : 'border-border text-foreground-muted hover:text-foreground hover:bg-background-tertiary'
                }`}
              >
                {id}
              </button>
            ))}
          </div>

          {/* Overlay preview container — simulates the input area */}
          <div className="relative h-[420px] bg-background-secondary rounded-lg border border-border flex flex-col">
            <div className="flex-1 flex items-center justify-center text-xs text-foreground-subtle">
              Chat area (overlays appear above the input)
            </div>

            {/* Simulated input area with overlay */}
            <div className="p-3 shrink-0 relative">
              {/* Overlays */}
              {activeOverlay === 'execution' && (
                <MockExecutionOverlay onClose={() => setActiveOverlay(null)} />
              )}
              {activeOverlay === 'toolConfirm' && (
                <MockToolConfirmOverlay onClose={() => setActiveOverlay(null)} />
              )}
              {activeOverlay === 'askUser' && (
                <MockAskUserOverlay onClose={() => setActiveOverlay(null)} />
              )}
              {activeOverlay === 'planView' && (
                <MockPlanViewOverlay onClose={() => setActiveOverlay(null)} />
              )}

              {/* Input mock */}
              <div className="rounded-xl border border-border bg-background-tertiary px-3.5 py-3 text-sm text-foreground-subtle/60">
                Ask something...
              </div>
            </div>
          </div>
        </Section>

        {/* ── Color Accents Reference ────────────────────────────────── */}
        <Section title="Accent Colors">
          <div className="grid grid-cols-4 gap-3">
            {(['amber', 'brand', 'blue', 'emerald'] as const).map((color) => (
              <div key={color} className="p-3 rounded-lg border border-border bg-background-secondary">
                <FloatingOverlayHeader
                  icon={Activity}
                  title={color}
                  accentColor={color}
                  badge="badge"
                  onClose={() => {}}
                />
              </div>
            ))}
          </div>
        </Section>
      </div>
    </div>
  )
}
