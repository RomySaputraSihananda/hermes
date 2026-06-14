import type { Config } from "tailwindcss";

const config: Config = {
  content: ["./app/**/*.{ts,tsx}", "./components/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        canvas:       "#010102",
        "s1":         "#0f1011",
        "s2":         "#141516",
        "s3":         "#18191a",
        "hl":         "#23252a",
        "hl-strong":  "#34343a",
        ink:          "#f7f8f8",
        "ink-md":     "#d0d6e0",
        "ink-sub":    "#8a8f98",
        "ink-ter":    "#62666d",
        accent:       "#d4a843",
        "accent-hover": "#f0c060",
        "accent-focus": "#c49833",
        "inv-canvas": "#ffffff",
        "inv-ink":    "#000000",
        bull:  "#27a644",
        bear:  "#e5484d",
        "bull-subtle": "rgba(39,166,68,0.12)",
        "bear-subtle": "rgba(229,72,77,0.12)",
      },
      borderRadius: {
        xs: "4px", sm: "6px", md: "8px", lg: "12px", xl: "16px", xxl: "24px", pill: "9999px",
      },
      fontFamily: {
        sans: ["Inter", "-apple-system", "system-ui", "Segoe UI", "sans-serif"],
        mono: ["'JetBrains Mono'", "'Geist Mono'", "ui-monospace", "SF Mono", "Menlo", "monospace"],
      },
      letterSpacing: {
        "tight-xl": "-0.18em",
        "tight-lg": "-0.032em",
        "tight-md": "-0.025em",
        "tight-sm": "-0.015em",
        "eyebrow":  "0.03em",
      },
      spacing: {
        xxs: "4px", xs: "8px", sm: "12px", md: "16px", lg: "24px",
        xl: "32px", xxl: "48px", section: "96px",
      },
    },
  },
  plugins: [],
};

export default config;
