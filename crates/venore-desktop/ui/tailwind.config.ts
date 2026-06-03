import type { Config } from "tailwindcss";

const config: Config = {
  darkMode: ["class"],
  content: [
    "./index.html",
    "./src/**/*.{ts,tsx,js,jsx}",
  ],
  theme: {
    extend: {
      colors: {
        // ============================================
        // VENORE BRAND COLORS
        // ============================================
        brand: {
          DEFAULT: "hsl(var(--brand))",
          hover: "hsl(var(--brand-hover))",
          muted: "hsl(var(--brand-muted))",
        },

        // ============================================
        // BACKGROUNDS (Dark Theme)
        // ============================================
        background: {
          DEFAULT: "hsl(var(--background))",
          secondary: "hsl(var(--background-secondary))",
          tertiary: "hsl(var(--background-tertiary))",
        },

        // ============================================
        // FOREGROUND (Text)
        // ============================================
        foreground: {
          DEFAULT: "hsl(var(--foreground))",
          muted: "hsl(var(--foreground-muted))",
          subtle: "hsl(var(--foreground-subtle))",
        },

        // ============================================
        // BORDERS
        // ============================================
        border: {
          DEFAULT: "hsl(var(--border))",
          hover: "hsl(var(--border-hover))",
        },

        // ============================================
        // SEMANTIC COLORS (Estados)
        // ============================================
        semantic: {
          success: "hsl(var(--semantic-success))",
          warning: "hsl(var(--semantic-warning))",
          error: "hsl(var(--semantic-error))",
          info: "hsl(var(--semantic-info))",
        },

        // ============================================
        // SHADCN COMPATIBILITY
        // Mapeo para componentes shadcn
        // ============================================
        primary: {
          DEFAULT: "hsl(var(--brand))",
          foreground: "hsl(var(--background))",
        },
        secondary: {
          DEFAULT: "hsl(var(--background-tertiary))",
          foreground: "hsl(var(--foreground))",
        },
        destructive: {
          DEFAULT: "hsl(var(--semantic-error))",
          foreground: "hsl(var(--foreground))",
        },
        muted: {
          DEFAULT: "hsl(var(--background-secondary))",
          foreground: "hsl(var(--foreground-muted))",
        },
        accent: {
          DEFAULT: "hsl(var(--background-tertiary))",
          foreground: "hsl(var(--foreground))",
        },
        card: {
          DEFAULT: "hsl(var(--background-secondary))",
          foreground: "hsl(var(--foreground))",
        },
        popover: {
          DEFAULT: "hsl(var(--background-secondary))",
          foreground: "hsl(var(--foreground))",
        },
        input: "hsl(var(--border))",
        ring: "hsl(var(--brand))",
      },

      borderRadius: {
        lg: "var(--radius)",
        md: "calc(var(--radius) - 2px)",
        sm: "calc(var(--radius) - 4px)",
      },

      fontFamily: {
        sans: ["Inter", "system-ui", "sans-serif"],
        mono: ["JetBrains Mono", "monospace"],
      },

      keyframes: {
        "accordion-down": {
          from: { height: "0" },
          to: { height: "var(--radix-accordion-content-height)" },
        },
        "accordion-up": {
          from: { height: "var(--radix-accordion-content-height)" },
          to: { height: "0" },
        },
      },
      animation: {
        "accordion-down": "accordion-down 0.2s ease-out",
        "accordion-up": "accordion-up 0.2s ease-out",
      },
    },
  },
  plugins: [require("tailwindcss-animate")],
};

export default config;
