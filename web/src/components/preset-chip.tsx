// Pill-shaped chip used by both the torrent-list filter strip
// (`preset-chip-strip`) and the config-page tab strip (`config.tsx`).
//
// `color` may be a hex (`#rrggbb`) or a CSS variable like `var(--foreground)`.
// Hex tints the active background; non-hex falls back to neutral gray.

import { Plus } from "lucide-react";
import { useEffect, useRef } from "react";

import { isHexColor, tintBackground, tintBorder } from "@/lib/preset-colors";
import { cn } from "@/lib/utils";

export interface PresetChipProps {
  active: boolean;
  onSelect: () => void;
  color: string;
  label: string;
  /** Replaces the color dot (e.g. Cog/Sparkles for special tabs). */
  icon?: React.ReactNode;
  /** Right-side counter (chip-strip totals). */
  count?: number | null;
  /** Auto-scroll into view on becoming active. Default `true`. */
  autoScroll?: boolean;
  ariaLabel?: string;
}

export function PresetChip({
  active,
  onSelect,
  color,
  label,
  icon,
  count,
  autoScroll = true,
  ariaLabel,
}: PresetChipProps) {
  const ref = useRef<HTMLButtonElement | null>(null);

  useEffect(() => {
    if (active && autoScroll && ref.current) {
      ref.current.scrollIntoView({
        block: "nearest",
        inline: "center",
        behavior: "smooth",
      });
    }
  }, [active, autoScroll]);

  const styleVars = isHexColor(color)
    ? ({
        "--chip-bg": tintBackground(color, 0.08),
        "--chip-border": tintBorder(color),
      } as React.CSSProperties)
    : ({
        "--chip-bg": "rgba(127, 127, 127, 0.08)",
        "--chip-border": "rgba(127, 127, 127, 0.35)",
      } as React.CSSProperties);

  return (
    <button
      ref={ref}
      type="button"
      onClick={onSelect}
      data-active={active}
      aria-label={ariaLabel}
      className={cn(
        "group relative inline-flex shrink-0 snap-start cursor-pointer items-center gap-2 rounded-full border px-3 py-1.5 transition-all md:px-3.5 md:py-1.5",
        "border-border/70 bg-card hover:bg-foreground/[0.04]",
        "data-[active=true]:border-[color:var(--chip-border)] data-[active=true]:bg-[color:var(--chip-bg)] data-[active=true]:shadow-[inset_0_0_0_1px_var(--chip-border)]",
      )}
      style={styleVars}
    >
      {icon ? (
        <span
          className={cn(
            "transition-colors",
            active ? "text-foreground" : "text-foreground/70",
          )}
        >
          {icon}
        </span>
      ) : (
        <span
          aria-hidden="true"
          className={cn(
            "size-2 shrink-0 rounded-full transition-transform",
            active && "scale-110",
          )}
          style={{ background: color }}
        />
      )}
      <span
        className={cn(
          "text-[12.5px] font-medium leading-none tracking-tight transition-colors",
          active ? "text-foreground" : "text-foreground/80",
        )}
      >
        {label}
      </span>
      {count != null && (
        <span
          className={cn(
            "font-mono text-[10.5px] tabular-nums leading-none transition-colors",
            active ? "text-foreground/85" : "text-muted-foreground/70",
          )}
        >
          {count}
        </span>
      )}
    </button>
  );
}

/** Dashed "+ New" companion to `PresetChip`, geometry-matched so it doesn't
 *  shift the strip height when toggled with an active chip. */
export function NewChip({
  onClick,
  label = "New",
  ariaLabel = "Create preset",
}: {
  onClick: () => void;
  label?: string;
  ariaLabel?: string;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-label={ariaLabel}
      className={cn(
        "inline-flex shrink-0 cursor-pointer items-center gap-2 rounded-full border border-dashed border-border/80 px-3 py-1.5 text-muted-foreground transition-colors hover:border-foreground/40 hover:text-foreground md:px-3.5 md:py-1.5",
      )}
    >
      <Plus className="size-3" strokeWidth={2} />
      <span className="text-[12.5px] font-medium leading-none tracking-tight">
        {label}
      </span>
    </button>
  );
}
