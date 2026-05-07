// Generic "review-and-save" confirmation dialog with a diff list.
// Used by Engine config save and per-preset policy save.

import { Save } from "lucide-react";

import { DiffList, type DiffListItem } from "@/components/diff-list";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

export function SaveConfirmDialog({
  open,
  onOpenChange,
  title,
  description,
  eyebrow,
  items,
  pending,
  confirmLabel = "Save changes",
  pendingLabel = "Saving…",
  onConfirm,
}: {
  open: boolean;
  onOpenChange: (v: boolean) => void;
  title: string;
  description?: string;
  eyebrow?: string;
  items: DiffListItem[];
  pending: boolean;
  confirmLabel?: string;
  pendingLabel?: string;
  onConfirm: () => void | Promise<void>;
}) {
  return (
    <Dialog open={open} onOpenChange={(v) => !pending && onOpenChange(v)}>
      <DialogContent className="sm:max-w-md md:max-w-lg [&>*]:min-w-0">
        <DialogHeader>
          {eyebrow && <span className="eyebrow-strong">{eyebrow}</span>}
          <DialogTitle className="text-base font-semibold">{title}</DialogTitle>
          {description && (
            <DialogDescription className="text-[12px]">
              {description}
            </DialogDescription>
          )}
        </DialogHeader>

        {items.length === 0 ? (
          <div className="rounded-md border border-dashed bg-card/30 p-6 text-center font-mono text-[11px] text-muted-foreground">
            No changes
          </div>
        ) : (
          <DiffList items={items} />
        )}

        <DialogFooter className="gap-2 sm:gap-2">
          <Button
            variant="ghost"
            className="h-9"
            onClick={() => onOpenChange(false)}
            disabled={pending}
          >
            Cancel
          </Button>
          <Button
            className="h-9 gap-1.5"
            onClick={onConfirm}
            disabled={pending || items.length === 0}
          >
            <Save className="size-3.5" strokeWidth={2} />
            {pending ? pendingLabel : confirmLabel}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
