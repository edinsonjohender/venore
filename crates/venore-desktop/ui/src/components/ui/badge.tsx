import * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";

import { cn } from "@/lib/utils";

/**
 * Badge Variants
 *
 * Customized for Venore Design System with semantic colors
 */
const badgeVariants = cva(
  "inline-flex items-center rounded border px-2.5 py-0.5 text-xs font-semibold transition-colors focus:outline-none focus:ring-2 focus:ring-brand focus:ring-offset-2",
  {
    variants: {
      variant: {
        // Success - Brand/Success color
        default:
          "border-transparent bg-semantic-success/10 text-semantic-success",

        // Warning
        warning:
          "border-transparent bg-semantic-warning/10 text-semantic-warning",

        // Error/Destructive
        destructive:
          "border-transparent bg-semantic-error/10 text-semantic-error",

        // Info
        info:
          "border-transparent bg-semantic-info/10 text-semantic-info",

        // Outline
        outline:
          "border-border text-foreground",

        // Secondary
        secondary:
          "border-transparent bg-background-tertiary text-foreground",
      },
    },
    defaultVariants: {
      variant: "default",
    },
  }
);

export interface BadgeProps
  extends React.HTMLAttributes<HTMLDivElement>,
    VariantProps<typeof badgeVariants> {}

function Badge({ className, variant, ...props }: BadgeProps) {
  return (
    <div className={cn(badgeVariants({ variant }), className)} {...props} />
  );
}

export { Badge, badgeVariants };
