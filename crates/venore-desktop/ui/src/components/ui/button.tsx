import * as React from "react";
import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";

import { cn } from "@/lib/utils";

/**
 * Button Variants
 *
 * Customized for Venore Design System with #01e8a2 brand color
 */
const buttonVariants = cva(
  "inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-lg text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-brand disabled:pointer-events-none disabled:opacity-50",
  {
    variants: {
      variant: {
        // Primary - Brand color
        default:
          "bg-brand hover:bg-brand-hover text-background shadow-sm",

        // Secondary - Tertiary background
        secondary:
          "bg-background-tertiary hover:bg-border-hover text-foreground border border-border",

        // Ghost - Transparent with hover
        ghost:
          "hover:bg-background-tertiary text-foreground-muted hover:text-foreground",

        // Destructive - Error color
        destructive:
          "bg-semantic-error/10 hover:bg-semantic-error/20 text-semantic-error",

        // Outline - Border only
        outline:
          "border border-border hover:border-border-hover hover:bg-background-tertiary text-foreground",

        // Link - No background
        link:
          "text-brand underline-offset-4 hover:underline",
      },
      size: {
        default: "h-10 px-4 py-2",
        sm: "h-8 rounded-md px-3 text-xs",
        lg: "h-12 rounded-lg px-6",
        icon: "h-10 w-10",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  }
);

export interface ButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {
  asChild?: boolean;
}

const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant, size, asChild = false, ...props }, ref) => {
    const Comp = asChild ? Slot : "button";
    return (
      <Comp
        className={cn(buttonVariants({ variant, size, className }))}
        ref={ref}
        {...props}
      />
    );
  }
);
Button.displayName = "Button";

export { Button, buttonVariants };
