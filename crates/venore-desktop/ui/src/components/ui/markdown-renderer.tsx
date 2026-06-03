// =============================================================================
// MarkdownRenderer - Reusable markdown component with Venore design tokens
// =============================================================================

import React from 'react'
import type { Components } from 'react-markdown'
import Markdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import { CodeBlock } from '@/components/ui/code-block'
import { cn } from '@/lib/utils'

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface MarkdownRendererProps {
  content: string
  className?: string
}

// -----------------------------------------------------------------------------
// Component overrides using Venore design tokens
// -----------------------------------------------------------------------------

const components: Components = {
  h1: ({ children }) => (
    <h1 className="text-lg font-semibold text-foreground mb-3">{children}</h1>
  ),
  h2: ({ children }) => (
    <h2 className="text-base font-semibold text-foreground mb-2">{children}</h2>
  ),
  h3: ({ children }) => (
    <h3 className="text-sm font-semibold text-foreground mb-1">{children}</h3>
  ),
  h4: ({ children }) => (
    <h4 className="text-sm font-semibold text-foreground mb-1">{children}</h4>
  ),
  p: ({ children }) => (
    <p className="text-sm text-foreground leading-relaxed mb-2 last:mb-0">{children}</p>
  ),
  a: ({ href, children }) => (
    <a href={href} className="text-brand hover:underline" target="_blank" rel="noopener noreferrer">
      {children}
    </a>
  ),
  ul: ({ children }) => (
    <ul className="text-sm space-y-1 pl-4 mb-2 list-disc">{children}</ul>
  ),
  ol: ({ children }) => (
    <ol className="text-sm space-y-1 pl-4 mb-2 list-decimal">{children}</ol>
  ),
  li: ({ children }) => (
    <li className="text-sm text-foreground">{children}</li>
  ),
  blockquote: ({ children }) => (
    <blockquote className="border-l-2 border-brand pl-3 text-foreground-muted italic mb-2">
      {children}
    </blockquote>
  ),
  // Fenced code blocks come through `pre > code.language-*`
  pre: ({ children }) => {
    // Extract language + code string from the child <code> element
    const codeChild = React.Children.toArray(children).find(
      (child): child is React.ReactElement =>
        React.isValidElement(child) && (child as React.ReactElement<{ className?: string }>).type === 'code',
    )

    if (codeChild) {
      const props = codeChild.props as { className?: string; children?: React.ReactNode }
      const langMatch = props.className?.match(/language-(\S+)/)
      const language = langMatch?.[1]
      // react-markdown passes children as string (or array with a single string)
      const codeStr = String(props.children ?? '').replace(/\n$/, '')
      return <CodeBlock code={codeStr} language={language} />
    }

    // Fallback: plain pre without a recognizable code child
    return (
      <pre className="font-mono text-xs bg-background p-3 rounded-lg border border-border overflow-x-auto mb-2">
        {children}
      </pre>
    )
  },
  // Inline code only (block code is caught by `pre` above)
  code: ({ className, children }) => {
    // If it has a language class, it's being rendered inside our `pre` handler — skip
    if (className?.includes('language-')) {
      return <code className={cn('font-mono text-xs', className)}>{children}</code>
    }
    return (
      <code className="font-mono text-xs bg-background-tertiary text-brand/80 px-1.5 py-0.5 rounded">
        {children}
      </code>
    )
  },
  table: ({ children }) => (
    <div className="overflow-x-auto mb-2 rounded-lg border border-border overflow-hidden">
      <table className="text-xs w-full">{children}</table>
    </div>
  ),
  tr: ({ children, ...props }) => (
    <tr className="even:bg-background-secondary/30" {...props}>{children}</tr>
  ),
  th: ({ children }) => (
    <th className="border-b border-border px-2 py-1 text-left font-semibold text-foreground bg-background-tertiary">
      {children}
    </th>
  ),
  td: ({ children }) => (
    <td className="border-b border-border/50 px-2 py-1 text-foreground">{children}</td>
  ),
  hr: () => <hr className="border-border my-3" />,
}

// -----------------------------------------------------------------------------
// Component
// -----------------------------------------------------------------------------

export function MarkdownRenderer({ content, className }: MarkdownRendererProps) {
  return (
    <div className={cn('markdown-content', className)}>
      <Markdown remarkPlugins={[remarkGfm]} components={components}>
        {content}
      </Markdown>
    </div>
  )
}
