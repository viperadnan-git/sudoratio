import { MoreHorizontal, Pause, Play, Radio, Trash2 } from "lucide-react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  useAnnounceTorrent,
  useDeleteTorrent,
  usePauseTorrent,
  useResumeTorrent,
} from "@/lib/queries";
import type { Torrent } from "@/lib/types";

export function TorrentActions({ t }: { t: Torrent }) {
  const pause = usePauseTorrent();
  const resume = useResumeTorrent();
  const del = useDeleteTorrent();
  const announce = useAnnounceTorrent();

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
        {isPaused ? (
          <DropdownMenuItem
            onClick={() => wrap(resume.mutateAsync, "Resumed")(ih)}
          >
            <Play className="size-3.5" strokeWidth={1.75} />
            Resume
          </DropdownMenuItem>
        ) : (
          <DropdownMenuItem
            onClick={() => wrap(pause.mutateAsync, "Paused")(ih)}
          >
            <Pause className="size-3.5" strokeWidth={1.75} />
            Pause
          </DropdownMenuItem>
        )}
        <DropdownMenuItem
          disabled={!isActive}
          onClick={() =>
            wrap(
              (v: { infoHash: string; event: "none" }) =>
                announce.mutateAsync(v),
              "Announce dispatched",
            )({ infoHash: ih, event: "none" })
          }
        >
          <Radio className="size-3.5" strokeWidth={1.75} />
          Announce now
        </DropdownMenuItem>
        <DropdownMenuSeparator />
        <DropdownMenuItem
          variant="destructive"
          onClick={() => wrap(del.mutateAsync, "Removed")(ih)}
        >
          <Trash2 className="size-3.5" strokeWidth={1.75} />
          Delete
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
