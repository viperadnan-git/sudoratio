// Bottom sheet of presets — used to pick a preset (reassign / on-add / delete-reassign).

import { Check, Lock } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetFooter,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";
import { usePresets, useProfiles } from "@/lib/queries";
import type { Preset } from "@/lib/types";
import { cn } from "@/lib/utils";

export function PresetPickerSheet({
  open,
  onOpenChange,
  selectedId,
  excludeId,
  onSelect,
  title = "Move to preset",
  description,
  /** When true, disable presets whose effective client profile differs from
   * the currently selected one (cross-client moves are rejected by the engine). */
  enforceClientMatch = false,
}: {
  open: boolean;
  onOpenChange: (v: boolean) => void;
  selectedId?: string | null;
  excludeId?: string;
  onSelect: (presetId: string) => void;
  title?: string;
  description?: string;
  enforceClientMatch?: boolean;
}) {
  const { data: presets, isLoading } = usePresets();
  const { data: profiles } = useProfiles();
  const items = (presets ?? []).filter((p) => p.id !== excludeId);

  const activeDefaultId = profiles?.find((p) => p.active)?.id ?? null;
  const resolveProfile = (p: Preset): string | null =>
    p.policy.client_profile_id ?? activeDefaultId;
  const currentPreset = selectedId
    ? ((presets ?? []).find((p) => p.id === selectedId) ?? null)
    : null;
  const currentProfile = currentPreset ? resolveProfile(currentPreset) : null;

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent
        side="bottom"
        className="rounded-t-xl border-t bg-background pb-[max(1rem,env(safe-area-inset-bottom))] sm:max-w-md sm:mx-auto"
      >
        <SheetHeader className="space-y-1">
          <span className="eyebrow-strong">Presets</span>
          <SheetTitle className="text-base font-semibold tracking-tight">
            {title}
          </SheetTitle>
          {description && (
            <SheetDescription className="text-[12px]">
              {description}
            </SheetDescription>
          )}
        </SheetHeader>

        <div className="-mx-1 mt-1 max-h-[55vh] space-y-1 overflow-y-auto px-1 pb-2">
          {isLoading && (
            <div className="px-3 py-6 text-center font-mono text-[11px] text-muted-foreground">
              › loading…
            </div>
          )}
          {!isLoading && items.length === 0 && (
            <div className="px-3 py-6 text-center font-mono text-[11px] text-muted-foreground">
              No other presets
            </div>
          )}
          {items.map((p) => {
            const active = p.id === selectedId;
            const mismatch =
              enforceClientMatch &&
              currentPreset != null &&
              resolveProfile(p) !== currentProfile;
            const disabled = mismatch;
            return (
              <button
                key={p.id}
                type="button"
                onClick={() => !disabled && onSelect(p.id)}
                disabled={disabled}
                data-active={active}
                title={
                  mismatch
                    ? "Different client profile — delete and re-add the torrent to switch identity"
                    : undefined
                }
                className={cn(
                  "group flex w-full items-center gap-3 rounded-md border border-transparent bg-transparent px-3 py-2.5 text-left transition-colors",
                  "enabled:cursor-pointer enabled:hover:bg-foreground/[0.04] enabled:active:bg-foreground/[0.08]",
                  "disabled:cursor-not-allowed disabled:opacity-45",
                  "data-[active=true]:bg-foreground/[0.05] data-[active=true]:border-border",
                )}
              >
                <span
                  aria-hidden="true"
                  className="size-3 shrink-0 rounded-full ring-1 ring-foreground/10"
                  style={{ background: p.color }}
                />
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2 text-[13px] font-medium leading-tight">
                    <span className="truncate">{p.name}</span>
                    {p.is_default && (
                      <span className="font-mono text-[9.5px] uppercase tracking-wider text-muted-foreground/70">
                        default
                      </span>
                    )}
                  </div>
                  <div className="mt-0.5 font-mono text-[10.5px] uppercase tracking-wider text-muted-foreground/65">
                    #{p.id} · {p.policy.min_upload_speed}–
                    {p.policy.max_upload_speed} KB/s ·{" "}
                    {p.policy.max_active_torrents} slots
                    {mismatch && (
                      <span className="ml-1.5 text-amber-600/80 dark:text-amber-400/80">
                        · different client
                      </span>
                    )}
                  </div>
                </div>
                {mismatch ? (
                  <Lock
                    className="size-3.5 shrink-0 text-muted-foreground/60"
                    strokeWidth={2}
                  />
                ) : active ? (
                  <Check
                    className="size-3.5 shrink-0 text-foreground/70"
                    strokeWidth={2}
                  />
                ) : null}
              </button>
            );
          })}
        </div>

        <SheetFooter className="mt-1 flex-row justify-end gap-2 sm:gap-2">
          <Button
            variant="ghost"
            size="sm"
            className="h-9"
            onClick={() => onOpenChange(false)}
          >
            Cancel
          </Button>
        </SheetFooter>
      </SheetContent>
    </Sheet>
  );
}
