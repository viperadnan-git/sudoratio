import type { Torrent } from "@/lib/types";
import { cn } from "@/lib/utils";

export function TorrentStatusBadge({
  t,
  className,
}: {
  t: Torrent;
  className?: string;
}) {
  const { color, label } = describe(t);
  return (
    <span
      className={cn(
        "inline-flex items-center font-mono text-[10px] font-semibold uppercase tracking-[0.14em]",
        className,
      )}
      style={{ color }}
    >
      {label}
    </span>
  );
}

function describe(t: Torrent): { color: string; label: string } {
  switch (t.state) {
    case "downloading":
      return { color: "var(--signal)", label: "Downloading" };
    case "seeding":
      return { color: "var(--success)", label: "Seeding" };
    case "queued":
      return {
        color: "var(--muted-foreground)",
        label: `Queued · #${t.queue_position + 1}`,
      };
    case "stopped":
      return stoppedDescription(t);
  }
}

function stoppedDescription(t: Torrent) {
  switch (t.reason) {
    case "upload_ratio":
      return { color: "var(--warn)", label: "Ratio capped" };
    case "no_leechers":
      return { color: "var(--warn)", label: "No leechers" };
    case "tracker_failed":
      return { color: "var(--destructive)", label: "Tracker failed" };
    default:
      return { color: "var(--muted-foreground)", label: "Paused" };
  }
}
