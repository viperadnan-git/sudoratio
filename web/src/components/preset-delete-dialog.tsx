// Delete preset confirmation. If torrents reference it, force a reassign target.

import { AlertTriangle } from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { useDeletePreset, usePresets, useTorrents } from "@/lib/queries";
import type { Preset } from "@/lib/types";
import { cn } from "@/lib/utils";

export function PresetDeleteDialog({
  open,
  onOpenChange,
  preset,
}: {
  open: boolean;
  onOpenChange: (v: boolean) => void;
  preset: Preset | null;
}) {
  const { data: torrents } = useTorrents({
    presetId: preset?.id,
    limit: 200,
  });
  const torrentCount = torrents?.total ?? 0;
  const { data: presets } = usePresets();
  const targets = (presets ?? []).filter((p) => p.id !== preset?.id);
  const [reassignTo, setReassignTo] = useState<string>("default");
  useEffect(() => {
    if (open) setReassignTo("default");
  }, [open]);

  const del = useDeletePreset();
  const busy = del.isPending;
  const needsReassign = torrentCount > 0;

  const onConfirm = async () => {
    if (!preset) return;
    try {
      await del.mutateAsync({
        id: preset.id,
        reassignTo: needsReassign ? reassignTo : undefined,
      });
      toast.success(`Preset "${preset.name}" deleted`);
      onOpenChange(false);
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "delete failed");
    }
  };

  return (
    <Dialog open={open} onOpenChange={(v) => !busy && onOpenChange(v)}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <span className="eyebrow-strong">Delete preset</span>
          <DialogTitle className="text-base font-semibold">
            Remove "{preset?.name}"?
          </DialogTitle>
          <DialogDescription className="text-[12px]">
            {needsReassign
              ? `This preset has ${torrentCount} torrent${torrentCount === 1 ? "" : "s"}. Pick a target preset to move them to.`
              : "No torrents are assigned. Safe to delete."}
          </DialogDescription>
        </DialogHeader>

        {needsReassign && (
          <div className="rounded-md border border-amber-500/30 bg-amber-500/[0.05] p-3">
            <div className="flex items-start gap-2">
              <AlertTriangle
                className="mt-[2px] size-3.5 shrink-0 text-amber-700 dark:text-amber-400"
                strokeWidth={1.75}
              />
              <div className="min-w-0 flex-1">
                <div className="text-[12px] font-medium leading-tight text-amber-700 dark:text-amber-400">
                  Reassign {torrentCount} torrent
                  {torrentCount === 1 ? "" : "s"} to:
                </div>
                <div className="mt-2 flex flex-wrap gap-1.5">
                  {targets.map((p) => (
                    <button
                      key={p.id}
                      type="button"
                      onClick={() => setReassignTo(p.id)}
                      data-active={reassignTo === p.id}
                      className={cn(
                        "inline-flex cursor-pointer items-center gap-1.5 rounded-full border bg-background px-2.5 py-1 transition-colors",
                        "data-[active=true]:bg-foreground/[0.06] data-[active=true]:border-foreground/40",
                      )}
                    >
                      <span
                        aria-hidden="true"
                        className="size-2 shrink-0 rounded-full"
                        style={{ background: p.color }}
                      />
                      <span className="text-[12px] font-medium leading-none">
                        {p.name}
                      </span>
                    </button>
                  ))}
                </div>
              </div>
            </div>
          </div>
        )}

        <DialogFooter className="gap-2 sm:gap-2">
          <Button
            variant="ghost"
            className="h-9"
            onClick={() => onOpenChange(false)}
            disabled={busy}
          >
            Cancel
          </Button>
          <Button
            variant="destructive"
            className="h-9"
            onClick={onConfirm}
            disabled={busy}
          >
            {busy ? "Deleting…" : "Delete preset"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
