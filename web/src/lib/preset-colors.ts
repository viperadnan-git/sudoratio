// Curated swatches for preset color selection. Validated against light + dark mode.
// Slate is `default`-tier; the rest are vivid enough to differentiate at-a-glance.

export interface Swatch {
  hex: string;
  label: string;
}

export const PRESET_SWATCHES: Swatch[] = [
  { hex: "#64748b", label: "Slate" },
  { hex: "#6366f1", label: "Indigo" },
  { hex: "#7c3aed", label: "Violet" },
  { hex: "#0ea5e9", label: "Sky" },
  { hex: "#06b6d4", label: "Cyan" },
  { hex: "#10b981", label: "Emerald" },
  { hex: "#84cc16", label: "Lime" },
  { hex: "#f59e0b", label: "Amber" },
  { hex: "#f97316", label: "Orange" },
  { hex: "#f43f5e", label: "Rose" },
  { hex: "#ec4899", label: "Pink" },
];

const DEFAULT_COLOR = "#64748b";

export function isHexColor(s: string): boolean {
  return /^#[0-9a-fA-F]{6}$/.test(s.trim());
}

export function normalizeHex(s: string): string {
  const t = s.trim();
  if (isHexColor(t)) return t.toLowerCase();
  return DEFAULT_COLOR;
}

/** Hex → rgba(...) at the given alpha. */
export function withAlpha(hex: string, alpha: number): string {
  const c = normalizeHex(hex);
  const r = parseInt(c.slice(1, 3), 16);
  const g = parseInt(c.slice(3, 5), 16);
  const b = parseInt(c.slice(5, 7), 16);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

/** Returns a CSS background that pairs well at the given strength (0..1). */
export function tintBackground(hex: string, strength: number): string {
  return withAlpha(hex, Math.max(0, Math.min(0.5, strength)));
}

export function tintBorder(hex: string): string {
  return withAlpha(hex, 0.35);
}
