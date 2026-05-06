// Number / size / time formatting helpers used across the dashboard.

const SIZE_UNITS = ["B", "KB", "MB", "GB", "TB"];

export function fmtBytes(bytes?: number | null): string {
  const n = bytes ?? 0;
  if (n === 0) return "0 B";
  let v = n;
  let i = 0;
  while (v >= 1024 && i < SIZE_UNITS.length - 1) {
    v /= 1024;
    i++;
  }
  if (i === 0) {
    return `${Math.round(v)} ${SIZE_UNITS[i]}`;
  }
  if (v >= 100) {
    return `${v.toFixed(0)} ${SIZE_UNITS[i]}`;
  }
  const trimmed = v.toFixed(2).replace(/\.?0+$/, "");
  return `${trimmed} ${SIZE_UNITS[i]}`;
}

export function fmtSpeed(bps?: number | null): string {
  return `${fmtBytes(bps)}/s`;
}

export function shortHash(hash?: string | null): string {
  if (!hash) return "";
  return hash.length > 12 ? `${hash.slice(0, 6)}…${hash.slice(-6)}` : hash;
}

export function fmtRelativeTime(unixMs?: number | null): string {
  if (!unixMs || unixMs === 0) return "never";
  const diff = Date.now() - unixMs;
  if (diff < 0) return "in the future";
  const s = Math.round(diff / 1000);
  if (s < 60) return `${s}s ago`;
  const m = Math.round(s / 60);
  if (m < 60) return `${m}m ago`;
  const h = Math.round(m / 60);
  if (h < 24) return `${h}h ago`;
  const d = Math.round(h / 24);
  return `${d}d ago`;
}

export function fmtCountdown(remainingMs: number): string {
  if (remainingMs <= 0) return "now";
  const totalSecs = Math.ceil(remainingMs / 1000);
  const m = Math.floor(totalSecs / 60);
  const s = totalSecs % 60;
  if (m === 0) return `${s}s`;
  return `${m}:${s.toString().padStart(2, "0")}`;
}

export function fmtRatio(
  uploaded?: number | null,
  size?: number | null,
): string {
  const u = uploaded ?? 0;
  const s = size ?? 0;
  if (s === 0) return "–";
  return (u / s).toFixed(2);
}

export function fmtDurationShort(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return "—";
  const s = Math.ceil(seconds);
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}h ${m % 60}m`;
  const d = Math.floor(h / 24);
  return `${d}d ${h % 24}h`;
}
