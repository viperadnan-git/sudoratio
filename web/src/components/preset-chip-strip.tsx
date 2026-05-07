// Horizontal chip strip showing All + each preset. Active chip uses preset color tint.
// Mobile: scrolls horizontally with snap. Desktop (md+): wraps.

import { Plus } from "lucide-react";
import { useEffect, useRef } from "react";
import { tintBackground, tintBorder } from "@/lib/preset-colors";
import { usePresetSelection } from "@/lib/preset-context";
import { usePresets } from "@/lib/queries";
import type { Preset } from "@/lib/types";
import { cn } from "@/lib/utils";

interface ChipProps {
  active: boolean;
  onSelect: () => void;
  color: string;
  label: string;
  count?: number | null;
}

function Chip({ active, onSelect, color, label, count }: ChipProps) {
  const ref = useRef<HTMLButtonElement | null>(null);

  useEffect(() => {
    if (active && ref.current) {
      ref.current.scrollIntoView({
        block: "nearest",
        inline: "center",
        behavior: "smooth",
      });
    }
  }, [active]);

  return (
    <button
      ref={ref}
      type="button"
      onClick={onSelect}
      data-active={active}
      className={cn(
        "group relative inline-flex shrink-0 snap-start cursor-pointer items-center gap-2 rounded-full border px-3 py-1.5 transition-all md:px-3.5 md:py-1.5",
        "border-border/70 bg-card hover:bg-foreground/[0.04]",
        "data-[active=true]:border-[color:var(--chip-border)] data-[active=true]:bg-[color:var(--chip-bg)] data-[active=true]:shadow-[inset_0_0_0_1px_var(--chip-border)]",
      )}
      style={
        {
          "--chip-bg": tintBackground(color, 0.08),
          "--chip-border": tintBorder(color),
        } as React.CSSProperties
      }
    >
      <span
        aria-hidden="true"
        className={cn(
          "size-2 shrink-0 rounded-full transition-transform",
          active && "scale-110",
        )}
        style={{ background: color }}
      />
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

export function PresetChipStrip({
  totals,
  onCreate,
}: {
  /** Optional torrent count per preset id (key "all" → grand total). */
  totals?: Record<string, number>;
  onCreate?: () => void;
}) {
  const { activeId, setActive } = usePresetSelection();
  const { data: presets } = usePresets();

  return (
    <div className="-mx-3 px-3 md:-mx-6 md:px-6">
      <div
        className={cn(
          "flex snap-x snap-mandatory items-center gap-1.5 overflow-x-auto overflow-y-hidden pb-1",
          "[scrollbar-width:none] [&::-webkit-scrollbar]:hidden",
          "md:flex-wrap md:gap-2 md:overflow-x-visible md:pb-0",
        )}
      >
        <Chip
          active={activeId === "all"}
          onSelect={() => setActive("all")}
          color="var(--foreground)"
          label="All"
          count={totals?.all}
        />
        {(presets ?? []).map((p: Preset) => (
          <Chip
            key={p.id}
            active={activeId === p.id}
            onSelect={() => setActive(p.id)}
            color={p.color}
            label={p.name}
            count={totals?.[p.id]}
          />
        ))}
        {onCreate && (
          <button
            type="button"
            onClick={onCreate}
            className={cn(
              "inline-flex shrink-0 cursor-pointer items-center gap-1 rounded-full border border-dashed border-border/80 px-3 py-1.5 text-muted-foreground transition-colors hover:border-foreground/40 hover:text-foreground",
            )}
            aria-label="Create preset"
          >
            <Plus className="size-3" strokeWidth={2} />
            <span className="font-mono text-[10.5px] uppercase tracking-wider">
              new
            </span>
          </button>
        )}
      </div>
    </div>
  );
}
