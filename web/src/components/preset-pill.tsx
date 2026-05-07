// Inline preset pill: color dot + name. Used in torrent rows.

import { cn } from "@/lib/utils";

export function PresetPill({
  color,
  name,
  className,
  onlyDot = false,
}: {
  color: string;
  name: string;
  className?: string;
  /** When true, render only the dot (used in dense layouts). */
  onlyDot?: boolean;
}) {
  if (onlyDot) {
    return (
      <span
        role="img"
        aria-label={name}
        title={name}
        className={cn(
          "inline-block size-2 shrink-0 rounded-full ring-1 ring-foreground/10",
          className,
        )}
        style={{ background: color }}
      />
    );
  }
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1.5 rounded-full bg-foreground/[0.04] px-1.5 py-[2px] font-mono text-[10px] uppercase tracking-wider text-foreground/75",
        className,
      )}
    >
      <span
        aria-hidden="true"
        className="size-1.5 shrink-0 rounded-full"
        style={{ background: color }}
      />
      <span className="truncate max-w-[10ch]">{name}</span>
    </span>
  );
}
