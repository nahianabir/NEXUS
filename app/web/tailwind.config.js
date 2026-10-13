/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        ink: {
          950: "#070a0e",
          900: "#0b1018",
          850: "#0f1620",
          800: "#141c28",
          700: "#1c2533",
          600: "#283246",
          500: "#3b4658",
        },
        cyan2: {
          400: "#67e0ff",
          500: "#3fc4ee",
          600: "#1fa5d3",
          700: "#0e7aa8",
        },
        violet2: {
          400: "#b89bff",
          500: "#9a78f0",
          600: "#7a52d4",
        },
        amber2: {
          400: "#ffc66e",
          500: "#f0a14a",
          600: "#c97c2a",
        },
        rose2: { 400: "#ff8aa3", 500: "#ef5f7c" },
      },
      fontFamily: {
        display: ["Inter", "ui-sans-serif", "system-ui", "Segoe UI", "Roboto", "sans-serif"],
        mono: ["JetBrains Mono", "ui-monospace", "Cascadia Mono", "Consolas", "monospace"],
      },
      boxShadow: {
        panel: "0 1px 0 rgba(255,255,255,0.04) inset, 0 0 0 1px rgba(255,255,255,0.04), 0 10px 40px rgba(0,0,0,0.4)",
        glow: "0 0 24px rgba(63,196,238,0.18)",
      },
      backdropBlur: { xs: "2px" },
      animation: {
        breathe: "breathe 5.5s ease-in-out infinite",
        spin12: "spin 12s linear infinite",
        spin24: "spin 24s linear infinite",
        pulse2: "pulse 4s ease-in-out infinite",
      },
      keyframes: {
        breathe: { "0%,100%": { opacity: "0.55" }, "50%": { opacity: "1" } },
      },
    },
  },
  plugins: [],
};
