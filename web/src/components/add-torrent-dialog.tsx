import { Check, ChevronDown, FileUp, Plus, X } from "lucide-react";
import { useRef, useState } from "react";
import { toast } from "sonner";
import { PresetPickerSheet } from "@/components/preset-picker-sheet";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { ApiError } from "@/lib/api";
import { fmtBytes } from "@/lib/format";
import { usePresetSelection } from "@/lib/preset-context";
import { useAddTorrent, usePresets } from "@/lib/queries";
import { cn } from "@/lib/utils";

type ItemStatus = "pending" | "uploading" | "ok" | "duplicate" | "error";

type Item = {
  id: string;
  file: File;
  status: ItemStatus;
  message?: string;
};

let nextItemId = 0;
const newItem = (file: File): Item => ({
  id: `${Date.now()}-${nextItemId++}`,
  file,
  status: "pending",
});

const onlyTorrents = (files: FileList | File[]) =>
  Array.from(files).filter(
    (f) =>
      f.name.toLowerCase().endsWith(".torrent") ||
      f.type === "application/x-bittorrent",
  );

export function AddTorrentDialog() {
  const [open, setOpen] = useState(false);
  const [items, setItems] = useState<Item[]>([]);
  const [hover, setHover] = useState(false);
  const [downloadBeforeSeed, setDownloadBeforeSeed] = useState(false);
  const [busy, setBusy] = useState(false);
  const inputRef = useRef<HTMLInputElement | null>(null);
  const add = useAddTorrent();
  const { activeId } = usePresetSelection();
  const { data: presets } = usePresets();
  const initialPresetId = activeId !== "all" ? activeId : "default";
  const [presetId, setPresetId] = useState<string>(initialPresetId);
  const [pickerOpen, setPickerOpen] = useState(false);
  const pickedPreset =
    (presets ?? []).find((p) => p.id === presetId) ??
    (presets ?? []).find((p) => p.is_default);

  const reset = () => {
    setItems([]);
    setDownloadBeforeSeed(false);
    setBusy(false);
    setPresetId(activeId !== "all" ? activeId : "default");
  };

  const appendFiles = (incoming: FileList | File[]) => {
    const torrents = onlyTorrents(incoming);
    const skipped = (incoming.length ?? 0) - torrents.length;
    if (skipped > 0) {
      toast.error(
        `Skipped ${skipped} non-torrent file${skipped === 1 ? "" : "s"}`,
      );
    }
    if (torrents.length === 0) return;
    setItems((prev) => {
      const seen = new Set(prev.map((i) => `${i.file.name}|${i.file.size}`));
      const fresh = torrents
        .filter((f) => !seen.has(`${f.name}|${f.size}`))
        .map(newItem);
      return [...prev, ...fresh];
    });
  };

  const removeItem = (id: string) =>
    setItems((prev) => prev.filter((i) => i.id !== id));

  const setItemStatus = (id: string, status: ItemStatus, message?: string) =>
    setItems((prev) =>
      prev.map((i) => (i.id === id ? { ...i, status, message } : i)),
    );

  const onSubmit = async () => {
    const queued = items.filter((i) => i.status !== "ok");
    if (queued.length === 0) return;
    setBusy(true);
    let added = 0;
    let duplicate = 0;
    let failed = 0;
    for (const item of queued) {
      setItemStatus(item.id, "uploading");
      try {
        await add.mutateAsync({
          file: item.file,
          downloadBeforeSeed,
          presetId,
        });
        setItemStatus(item.id, "ok");
        added += 1;
      } catch (e) {
        if (e instanceof ApiError && e.status === 409) {
          setItemStatus(item.id, "duplicate", "Already added");
          duplicate += 1;
        } else {
          setItemStatus(
            item.id,
            "error",
            e instanceof Error ? e.message : "add failed",
          );
          failed += 1;
        }
      }
    }
    setBusy(false);

    const parts: string[] = [];
    if (added) parts.push(`${added} added`);
    if (duplicate) parts.push(`${duplicate} already present`);
    if (failed) parts.push(`${failed} failed`);
    const summary = parts.join(" · ");

    if (failed === 0) {
      toast.success(summary || "Done");
      setOpen(false);
      reset();
    } else {
      toast.error(summary);
    }
  };

  const totalBytes = items.reduce((acc, i) => acc + i.file.size, 0);
  const remainingCount = items.filter((i) => i.status !== "ok").length;
  const submitLabel = busy
    ? "Adding…"
    : items.length <= 1
      ? "Add torrent"
      : `Add ${remainingCount} torrents`;

  return (
    <Dialog
      open={open}
      onOpenChange={(v) => {
        if (busy) return;
        setOpen(v);
        if (!v) reset();
      }}
    >
      <DialogTrigger asChild>
        <Button size="sm" className="h-8 gap-1.5 px-3 text-[12px]">
          <Plus className="size-3.5" strokeWidth={2} />
          Add torrent
        </Button>
      </DialogTrigger>

      <DialogContent className="sm:max-w-md md:max-w-lg lg:max-w-2xl [&>*]:min-w-0">
        <DialogHeader>
          <span className="eyebrow-strong">
            {items.length === 0
              ? "New torrent"
              : `Queue · ${items.length} · ${fmtBytes(totalBytes)}`}
          </span>
          <DialogTitle className="text-base font-semibold">
            Drop `.torrent` files to start announcing
          </DialogTitle>
          <DialogDescription className="text-[12px]">
            Files are added one-by-one in order. Multi-select is supported.
          </DialogDescription>
        </DialogHeader>

        <input
          ref={inputRef}
          type="file"
          accept=".torrent,application/x-bittorrent"
          multiple
          className="hidden"
          onChange={(e) => {
            if (e.target.files) appendFiles(e.target.files);
            e.target.value = "";
          }}
        />

        <button
          type="button"
          onClick={() => inputRef.current?.click()}
          onDragEnter={(e) => {
            e.preventDefault();
            setHover(true);
          }}
          onDragOver={(e) => {
            e.preventDefault();
            setHover(true);
          }}
          onDragLeave={() => setHover(false)}
          onDrop={(e) => {
            e.preventDefault();
            setHover(false);
            if (e.dataTransfer.files) appendFiles(e.dataTransfer.files);
          }}
          disabled={busy}
          className={cn(
            "group relative flex w-full flex-col items-center justify-center gap-2 rounded-md border border-dashed border-border bg-card/40 px-4 py-6 text-center transition-colors",
            hover && "border-signal bg-signal/5",
            items.length > 0 && !hover && "border-foreground/25",
            busy && "cursor-not-allowed opacity-50",
          )}
        >
          <FileUp
            className={cn(
              "size-6 text-muted-foreground transition-colors",
              hover && "text-signal",
            )}
            strokeWidth={1.5}
          />
          <div className="space-y-0.5">
            <div className="text-[13px] font-medium">
              {items.length === 0
                ? "Click or drop files here"
                : "Append more to the queue"}
            </div>
            <div className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground">
              ·torrent · multi-select supported
            </div>
          </div>
        </button>

        {items.length > 0 && (
          <ol
            className={cn(
              "min-w-0 space-y-px overflow-hidden rounded-md border border-border bg-card/40",
              items.length > 4 && "max-h-64 overflow-y-auto",
            )}
          >
            {items.map((item) => (
              <ItemRow
                key={item.id}
                item={item}
                onRemove={busy ? undefined : () => removeItem(item.id)}
              />
            ))}
          </ol>
        )}

        <button
          type="button"
          onClick={() => setPickerOpen(true)}
          disabled={busy}
          className={cn(
            "flex w-full cursor-pointer items-center justify-between gap-3 rounded-md border bg-card px-3 py-2.5 text-left transition-colors hover:bg-foreground/[0.03]",
          )}
        >
          <div className="flex min-w-0 flex-1 items-center gap-2.5">
            <span
              aria-hidden="true"
              className="size-2.5 shrink-0 rounded-full ring-1 ring-foreground/10"
              style={{ background: pickedPreset?.color ?? "#64748b" }}
            />
            <div className="min-w-0">
              <div className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground/70">
                Preset
              </div>
              <div className="text-[13px] font-medium leading-tight">
                {pickedPreset?.name ?? "Default"}
              </div>
            </div>
          </div>
          <ChevronDown
            className="size-3.5 shrink-0 text-muted-foreground"
            strokeWidth={2}
          />
        </button>

        <PresetPickerSheet
          open={pickerOpen}
          onOpenChange={setPickerOpen}
          selectedId={presetId}
          onSelect={(id) => {
            setPresetId(id);
            setPickerOpen(false);
          }}
          title="Choose preset"
          description="Applies to every file in this batch."
        />

        <div className="flex min-w-0 items-center gap-2.5">
          <Checkbox
            id="download-before-seed"
            checked={downloadBeforeSeed}
            onCheckedChange={(v) => setDownloadBeforeSeed(!!v)}
            disabled={busy}
          />
          <Label
            htmlFor="download-before-seed"
            className="cursor-pointer font-mono text-[12px] leading-none"
          >
            Download before seed
          </Label>
          <span className="font-mono text-[11px] text-muted-foreground">
            — applies to every file
          </span>
        </div>

        <DialogFooter className="gap-2 sm:gap-2">
          <Button
            variant="ghost"
            className="h-9"
            onClick={() => setOpen(false)}
            disabled={busy}
          >
            Cancel
          </Button>
          <Button
            className="h-9"
            disabled={items.length === 0 || busy}
            onClick={onSubmit}
          >
            {submitLabel}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

type ItemRowProps = {
  item: Item;
  onRemove?: () => void;
};

function ItemRow({ item, onRemove }: ItemRowProps) {
  return (
    <li
      className={cn(
        "group flex min-w-0 items-center gap-2.5 px-3 py-2 transition-colors",
        item.status === "uploading" && "bg-signal/5",
        item.status === "ok" && "bg-success/5",
        item.status === "error" && "bg-destructive/5",
      )}
    >
      <StatusDot status={item.status} />

      <div className="min-w-0 flex-1 overflow-hidden">
        <div
          className="num block w-full truncate text-[12px] font-medium"
          title={item.file.name}
        >
          {item.file.name}
        </div>
        <div className="flex min-w-0 items-center gap-1.5 overflow-hidden font-mono text-[10px] uppercase tracking-wider text-muted-foreground">
          <span className="num shrink-0">{fmtBytes(item.file.size)}</span>
          {item.message && (
            <>
              <span className="shrink-0 text-muted-foreground/50">·</span>
              <span
                className={cn(
                  "min-w-0 flex-1 truncate normal-case",
                  item.status === "duplicate" && "text-muted-foreground",
                  item.status === "error" && "text-destructive",
                )}
                title={item.message}
              >
                {item.message}
              </span>
            </>
          )}
        </div>
      </div>

      {onRemove && item.status !== "ok" && item.status !== "uploading" && (
        <button
          type="button"
          onClick={onRemove}
          aria-label={`Remove ${item.file.name}`}
          className="shrink-0 rounded p-1 text-muted-foreground/60 opacity-0 transition-all hover:bg-foreground/5 hover:text-foreground group-hover:opacity-100"
        >
          <X className="size-3.5" strokeWidth={1.75} />
        </button>
      )}
    </li>
  );
}

function StatusDot({ status }: { status: ItemStatus }) {
  if (status === "uploading") {
    return (
      <span className="relative inline-flex size-2 shrink-0 items-center justify-center text-signal">
        <span className="absolute inline-flex size-2 animate-ping rounded-full bg-signal/60" />
        <span className="relative inline-flex size-1.5 rounded-full bg-signal" />
      </span>
    );
  }
  if (status === "ok") {
    return (
      <span className="inline-flex size-2 shrink-0 items-center justify-center text-success">
        <Check className="size-3" strokeWidth={3} />
      </span>
    );
  }
  if (status === "duplicate") {
    return (
      <span className="inline-flex size-2 shrink-0 rounded-full border border-muted-foreground/60 bg-transparent" />
    );
  }
  if (status === "error") {
    return (
      <span className="inline-flex size-2 shrink-0 rounded-full bg-destructive" />
    );
  }
  return (
    <span className="inline-flex size-1.5 shrink-0 rounded-full bg-muted-foreground/40" />
  );
}
