import * as React from "react";

import { cn } from "@/lib/utils";

/**
 * Input Component
 *
 * Customized for Venore Design System
 * - Background: background-secondary
 * - Border: border
 * - Focus: brand ring
 */
export interface InputProps
  extends React.InputHTMLAttributes<HTMLInputElement> {}

const Input = React.forwardRef<HTMLInputElement, InputProps>(
  ({ className, type, ...props }, ref) => {
    return (
      <input
        type={type}
        className={cn(
          "flex h-10 w-full rounded-lg border border-border bg-background-secondary px-3 py-2 text-sm text-foreground",
          "placeholder:text-foreground-subtle",
          "focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-brand focus-visible:border-brand",
          "disabled:cursor-not-allowed disabled:opacity-50",
          "transition-colors",
          className
        )}
        ref={ref}
        {...props}
      />
    );
  }
);
Input.displayName = "Input";

export { Input };
