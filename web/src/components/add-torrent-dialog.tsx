import { FileUp, Plus } from "lucide-react";
import { useRef, useState } from "react";
import { toast } from "sonner";
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
import { useAddTorrent } from "@/lib/queries";
import { cn } from "@/lib/utils";

export function AddTorrentDialog() {
  const [open, setOpen] = useState(false);
  const [file, setFile] = useState<File | null>(null);
  const [hover, setHover] = useState(false);
  const [downloadBeforeSeed, setDownloadBeforeSeed] = useState(false);
  const inputRef = useRef<HTMLInputElement | null>(null);
  const add = useAddTorrent();

  const onAdd = async () => {
    if (!file) return;
    try {
      const res = await add.mutateAsync({ file, downloadBeforeSeed });
      toast.success(`Added ${res.info_hash.slice(0, 10)}…`);
      setOpen(false);
      setFile(null);
      setDownloadBeforeSeed(false);
    } catch (e) {
      if (e instanceof ApiError && e.status === 409) {
        toast.error("Already added");
      } else {
        toast.error(e instanceof Error ? e.message : "add failed");
      }
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(v) => {
        setOpen(v);
        if (!v) {
          setFile(null);
          setDownloadBeforeSeed(false);
        }
      }}
    >
      <DialogTrigger asChild>
        <Button size="sm" className="h-8 gap-1.5 px-3 text-[12px]">
          <Plus className="size-3.5" strokeWidth={2} />
          Add torrent
        </Button>
      </DialogTrigger>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <span className="eyebrow-strong">New torrent</span>
          <DialogTitle className="text-base font-semibold">
            Drop a `.torrent` to start announcing
          </DialogTitle>
          <DialogDescription className="text-[12px]">
            The engine spawns the announce loop immediately — no manual start.
          </DialogDescription>
        </DialogHeader>

        <input
          ref={inputRef}
          type="file"
          accept=".torrent,application/x-bittorrent"
          className="hidden"
          onChange={(e) => setFile(e.target.files?.[0] ?? null)}
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
            const f = e.dataTransfer.files?.[0];
            if (f) setFile(f);
          }}
          className={cn(
            "group relative flex w-full flex-col items-center justify-center gap-3 rounded-md border border-dashed border-border bg-card/40 px-4 py-8 text-center transition-colors",
            hover && "border-signal bg-signal/5",
            file && "border-foreground/30",
          )}
        >
          <FileUp
            className={cn(
              "size-7 text-muted-foreground transition-colors",
              hover && "text-signal",
            )}
            strokeWidth={1.5}
          />
          {file ? (
            <div className="space-y-1">
              <div
                className="num max-w-[24ch] truncate text-[13px] font-medium"
                title={file.name}
              >
                {file.name}
              </div>
              <div className="num text-[11px] text-muted-foreground">
                {fmtBytes(file.size)}
              </div>
            </div>
          ) : (
            <div className="space-y-1">
              <div className="text-[13px] font-medium">
                Click or drop a file here
              </div>
              <div className="font-mono text-[11px] uppercase tracking-wider text-muted-foreground">
                ·torrent
              </div>
            </div>
          )}
        </button>

        <div className="flex items-center gap-2.5">
          <Checkbox
            id="download-before-seed"
            checked={downloadBeforeSeed}
            onCheckedChange={(v) => setDownloadBeforeSeed(!!v)}
          />
          <Label
            htmlFor="download-before-seed"
            className="cursor-pointer font-mono text-[12px] leading-none"
          >
            Download before seed
          </Label>
          <span className="font-mono text-[11px] text-muted-foreground">
            — simulates download phase first
          </span>
        </div>

        <DialogFooter className="gap-2 sm:gap-2">
          <Button
            variant="ghost"
            className="h-9"
            onClick={() => setOpen(false)}
          >
            Cancel
          </Button>
          <Button
            className="h-9"
            disabled={!file || add.isPending}
            onClick={onAdd}
          >
            {add.isPending ? "Adding…" : "Add torrent"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
