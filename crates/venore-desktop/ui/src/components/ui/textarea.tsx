import * as React from "react";

import { cn } from "@/lib/utils";

/**
 * Textarea Component
 *
 * Customized for Venore Design System
 * - Background: background-secondary
 * - Border: border
 * - Focus: brand ring
 */
export interface TextareaProps
  extends React.TextareaHTMLAttributes<HTMLTextAreaElement> {}

const Textarea = React.forwardRef<HTMLTextAreaElement, TextareaProps>(
  ({ className, ...props }, ref) => {
    return (
      <textarea
        className={cn(
          "flex min-h-[80px] w-full rounded-lg border border-border bg-background-secondary px-3 py-2 text-sm text-foreground",
          "placeholder:text-foreground-subtle",
          "focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-brand focus-visible:border-brand",
          "disabled:cursor-not-allowed disabled:opacity-50",
          "transition-colors resize-none",
          className
        )}
        ref={ref}
        {...props}
      />
    );
  }
);
Textarea.displayName = "Textarea";

export { Textarea };
