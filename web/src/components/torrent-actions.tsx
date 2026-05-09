import {
  AlertTriangle,
  MoreHorizontal,
  MoveRight,
  Pause,
  Play,
  Radio,
  Trash2,
} from "lucide-react";
import { type ReactNode, useState } from "react";
import { toast } from "sonner";

import { PresetPickerSheet } from "@/components/preset-picker-sheet";
import { Button } from "@/components/ui/button";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  useAnnounceTorrent,
  useAssignTorrentPreset,
  useDeleteTorrent,
  usePauseTorrent,
  useResumeTorrent,
} from "@/lib/queries";
import type { Torrent } from "@/lib/types";

type ConfirmKind = "announce" | "delete";

type ActionItem = {
  key: string;
  label: string;
  icon: typeof Pause;
  onSelect: () => void;
  disabled?: boolean;
  variant?: "default" | "destructive";
};

export type TorrentMenu = {
  items: Array<ActionItem | "separator">;
  dialogs: ReactNode;
};

export function useTorrentMenu(t: Torrent): TorrentMenu | null {
  const pause = usePauseTorrent();
  const resume = useResumeTorrent();
  const del = useDeleteTorrent();
  const announce = useAnnounceTorrent();
  const assign = useAssignTorrentPreset();
  const [confirm, setConfirm] = useState<ConfirmKind | null>(null);
  const [pickerOpen, setPickerOpen] = useState(false);

  if (!t.info_hash) return null;
  const ih = t.info_hash;
  const isPaused = t.state === "stopped";
  const isActive = t.state === "downloading" || t.state === "seeding";

  const wrap =
    <T,>(fn: (v: T) => Promise<unknown>, msg: string) =>
    async (v: T) => {
      try {
        await fn(v);
        toast.success(msg);
      } catch (e) {
        toast.error(e instanceof Error ? e.message : "request failed");
      }
    };

  const onConfirmAnnounce = async () => {
    await wrap(
      (v: { infoHash: string; event: "none" }) => announce.mutateAsync(v),
      "Announce dispatched",
    )({ infoHash: ih, event: "none" });
    setConfirm(null);
  };

  const onConfirmDelete = async () => {
    await wrap(del.mutateAsync, "Removed")(ih);
    setConfirm(null);
  };

  const items: Array<ActionItem | "separator"> = [
    isPaused
      ? {
          key: "resume",
          label: "Resume",
          icon: Play,
          onSelect: () => wrap(resume.mutateAsync, "Resumed")(ih),
        }
      : {
          key: "pause",
          label: "Pause",
          icon: Pause,
          onSelect: () => wrap(pause.mutateAsync, "Paused")(ih),
        },
    {
      key: "announce",
      label: "Announce now",
      icon: Radio,
      onSelect: () => setConfirm("announce"),
      disabled: !isActive,
    },
    {
      key: "preset",
      label: "Change preset",
      icon: MoveRight,
      onSelect: () => setPickerOpen(true),
    },
    "separator",
    {
      key: "delete",
      label: "Delete",
      icon: Trash2,
      onSelect: () => setConfirm("delete"),
      variant: "destructive",
    },
  ];

  const dialogs = (
    <>
      <PresetPickerSheet
        open={pickerOpen}
        onOpenChange={setPickerOpen}
        selectedId={t.preset_id}
        enforceClientMatch
        onSelect={async (presetId) => {
          if (presetId === t.preset_id) {
            setPickerOpen(false);
            return;
          }
          try {
            await assign.mutateAsync({ infoHash: ih, presetId });
            toast.success(`Moved to ${presetId}`);
          } catch (e) {
            toast.error(e instanceof Error ? e.message : "move failed");
          }
          setPickerOpen(false);
        }}
        title={`Move "${t.name}"`}
        description={`Currently in #${t.preset_id}`}
      />
      <ConfirmDialog
        kind="announce"
        open={confirm === "announce"}
        torrentName={t.name}
        pending={announce.isPending}
        onCancel={() => setConfirm(null)}
        onConfirm={onConfirmAnnounce}
      />
      <ConfirmDialog
        kind="delete"
        open={confirm === "delete"}
        torrentName={t.name}
        pending={del.isPending}
        onCancel={() => setConfirm(null)}
        onConfirm={onConfirmDelete}
      />
    </>
  );

  return { items, dialogs };
}

export function TorrentActionsKebab({ menu }: { menu: TorrentMenu }) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          variant="ghost"
          size="icon"
          className="size-7"
          aria-label="Torrent actions"
          onClick={(e) => e.stopPropagation()}
        >
          <MoreHorizontal className="size-4" strokeWidth={1.75} />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent
        align="end"
        className="min-w-[12rem] font-mono text-[12px]"
        onClick={(e) => e.stopPropagation()}
      >
        {menu.items.map((it, i) =>
          it === "separator" ? (
            // biome-ignore lint/suspicious/noArrayIndexKey: menu structure is static
            <DropdownMenuSeparator key={`sep-${i}`} />
          ) : (
            <DropdownMenuItem
              key={it.key}
              variant={it.variant}
              disabled={it.disabled}
              onClick={it.onSelect}
            >
              <it.icon className="size-3.5" strokeWidth={1.75} />
              {it.label}
            </DropdownMenuItem>
          ),
        )}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

export function TorrentRowContextMenu({
  menu,
  children,
}: {
  menu: TorrentMenu;
  children: ReactNode;
}) {
  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>{children}</ContextMenuTrigger>
      <ContextMenuContent className="min-w-[12rem] font-mono text-[12px]">
        {menu.items.map((it, i) =>
          it === "separator" ? (
            // biome-ignore lint/suspicious/noArrayIndexKey: menu structure is static
            <ContextMenuSeparator key={`sep-${i}`} />
          ) : (
            <ContextMenuItem
              key={it.key}
              variant={it.variant}
              disabled={it.disabled}
              onSelect={it.onSelect}
            >
              <it.icon className="size-3.5" strokeWidth={1.75} />
              {it.label}
            </ContextMenuItem>
          ),
        )}
      </ContextMenuContent>
    </ContextMenu>
  );
}

export function TorrentActions({ t }: { t: Torrent }) {
  const menu = useTorrentMenu(t);
  if (!menu) return null;
  return (
    <>
      <TorrentActionsKebab menu={menu} />
      {menu.dialogs}
    </>
  );
}

function ConfirmDialog({
  kind,
  open,
  torrentName,
  pending,
  onCancel,
  onConfirm,
}: {
  kind: ConfirmKind;
  open: boolean;
  torrentName: string;
  pending: boolean;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  const meta =
    kind === "delete"
      ? {
          eyebrow: "Danger · Delete",
          title: "Delete torrent?",
          description:
            "Removes the torrent from the engine and sends a final stopped announce. Persisted history is purged. This cannot be undone.",
          confirmLabel: "Delete",
          confirmPending: "Deleting…",
          confirmVariant: "destructive" as const,
          icon: <AlertTriangle className="size-3.5" strokeWidth={2} />,
        }
      : {
          eyebrow: "Action · Announce",
          title: "Announce now?",
          description:
            "Triggers an immediate tracker announce outside the normal interval. Use sparingly — some private trackers throttle or warn on excessive manual announces.",
          confirmLabel: "Announce",
          confirmPending: "Dispatching…",
          confirmVariant: "default" as const,
          icon: <Radio className="size-3.5" strokeWidth={2} />,
        };

  return (
    <Dialog open={open} onOpenChange={(v) => !v && !pending && onCancel()}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <span className="eyebrow-strong">{meta.eyebrow}</span>
          <DialogTitle className="text-base font-semibold">
            {meta.title}
          </DialogTitle>
          <DialogDescription className="text-[12px]">
            {meta.description}
          </DialogDescription>
        </DialogHeader>
        <div className="rounded-md border border-border/70 bg-muted/30 px-3 py-2">
          <div className="font-mono text-[10.5px] uppercase tracking-wider text-muted-foreground/70">
            Torrent
          </div>
          <div className="mt-0.5 truncate text-[12.5px] font-medium">
            {torrentName}
          </div>
        </div>
        <DialogFooter>
          <Button
            type="button"
            variant="ghost"
            onClick={onCancel}
            disabled={pending}
          >
            Cancel
          </Button>
          <Button
            type="button"
            variant={meta.confirmVariant}
            onClick={onConfirm}
            disabled={pending}
            className="gap-1.5"
          >
            {meta.icon}
            {pending ? meta.confirmPending : meta.confirmLabel}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
