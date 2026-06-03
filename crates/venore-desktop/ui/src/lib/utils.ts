import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

/**
 * Merge Tailwind CSS classes with clsx
 *
 * This utility combines clsx (for conditional classes) with tailwind-merge
 * (to properly merge Tailwind classes and handle conflicts).
 *
 * @example
 * cn("px-4 py-2", condition && "bg-brand", "text-foreground")
 */
export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}
