// Horizontal chip strip showing All + each preset. Active chip uses preset color tint.
// Mobile: scrolls horizontally with snap. Desktop (md+): wraps.

import { NewChip, PresetChip } from "@/components/preset-chip";
import { usePresetSelection } from "@/lib/preset-context";
import { usePresets } from "@/lib/queries";
import type { Preset } from "@/lib/types";
import { cn } from "@/lib/utils";

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
        <PresetChip
          active={activeId === "all"}
          onSelect={() => setActive("all")}
          color="var(--foreground)"
          label="All"
          count={totals?.all}
        />
        {(presets ?? []).map((p: Preset) => (
          <PresetChip
            key={p.id}
            active={activeId === p.id}
            onSelect={() => setActive(p.id)}
            color={p.color}
            label={p.name}
            count={totals?.[p.id]}
          />
        ))}
        {onCreate && <NewChip onClick={onCreate} />}
      </div>
    </div>
  );
}
