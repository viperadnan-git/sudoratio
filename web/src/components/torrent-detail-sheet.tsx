import {
  Check,
  ChevronDown,
  ChevronRight,
  Copy,
  HardDrive,
  RefreshCw,
  Scale,
  Timer,
  TrendingDown,
  TrendingUp,
  Upload,
  Users,
  X,
} from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";

import { TorrentActions } from "@/components/torrent-actions";
import { TorrentStatusBadge } from "@/components/torrent-status-badge";
import { Button } from "@/components/ui/button";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";
import { useNow } from "@/hooks/use-now";
import {
  fmtBytes,
  fmtCountdown,
  fmtRatio,
  fmtRelativeTime,
  fmtSpeed,
} from "@/lib/format";
import { useTorrent, useTorrentAnnounces } from "@/lib/queries";
import type { AnnouncesPage, AnnounceTrace, Torrent } from "@/lib/types";
import { cn } from "@/lib/utils";

interface Props {
  infoHash: string | null;
  onClose: () => void;
}

const ANNOUNCES_PAGE_SIZE = 25;

export function TorrentDetailSheet({ infoHash, onClose }: Props) {
  const open = !!infoHash;
  const torrent = useTorrent(infoHash ?? undefined);
  const [page, setPage] = useState(0);
  // biome-ignore lint/correctness/useExhaustiveDependencies: reset paging on torrent switch
  useEffect(() => {
    setPage(0);
  }, [infoHash]);
  const announces = useTorrentAnnounces(
    infoHash ?? undefined,
    ANNOUNCES_PAGE_SIZE,
    page * ANNOUNCES_PAGE_SIZE,
  );
  const t = torrent.data;

  return (
    <Sheet open={open} onOpenChange={(v) => !v && onClose()}>
      <SheetContent
        side="right"
        className={cn(
          "flex flex-col gap-0 p-0",
          "!w-full sm:!w-3/4 sm:!max-w-[min(640px,90vw)]",
        )}
        showCloseButton={false}
      >
        {/* ── HEADER ── */}
        <SheetHeader className="flex-row items-start gap-3 space-y-0 border-b bg-card/40 p-4">
          <div className="min-w-0 flex-1">
            <div className="eyebrow mb-1.5">Torrent</div>
            <SheetTitle className="truncate text-base font-semibold leading-tight">
              {t?.name ?? (torrent.isLoading ? "Loading…" : "Not found")}
            </SheetTitle>
            <SheetDescription asChild>
              <div className="mt-1 flex items-center gap-2 font-mono text-[11px] text-muted-foreground">
                {t?.info_hash ? (
                  <>
                    <span className="truncate">{t.info_hash}</span>
                    <CopyButton value={t.info_hash} label="info-hash" />
                  </>
                ) : (
                  <span>—</span>
                )}
              </div>
            </SheetDescription>
          </div>
          <Button
            variant="ghost"
            size="icon"
            onClick={onClose}
            className="-mr-1 shrink-0"
            aria-label="Close"
          >
            <X className="size-4" strokeWidth={1.75} />
          </Button>
        </SheetHeader>

        {/* ── BODY (scrolls) ── */}
        <div className="min-h-0 flex-1 overflow-y-auto">
          {!t ? (
            <p className="p-6 text-sm text-muted-foreground">
              {torrent.isLoading ? "Loading torrent…" : "Torrent not found."}
            </p>
          ) : (
            <div className="flex flex-col">
              {/* Status row */}
              <div className="flex flex-wrap items-center gap-x-3 gap-y-2 border-b px-4 py-3">
                <TorrentStatusBadge t={t} />
                {t.download_before_seed && (
                  <span className="eyebrow">DL-FIRST</span>
                )}
                {t.state === "queued" && (
                  <span className="eyebrow">QUEUE #{t.queue_position + 1}</span>
                )}
                <span className="ml-auto">
                  <TorrentActions t={t} />
                </span>
              </div>

              <MetricsGrid t={t} />

              <TrackersList tiers={t.trackers.tiers} />

              <AnnouncesConsole
                page={announces.data ?? null}
                pageIndex={page}
                pageSize={ANNOUNCES_PAGE_SIZE}
                onPageChange={setPage}
                isLoading={announces.isLoading}
                isFetching={announces.isFetching}
              />
            </div>
          )}
        </div>
      </SheetContent>
    </Sheet>
  );
}

/* ───────────────────────────── metrics grid ─────────────────────────── */

function MetricsGrid({ t }: { t: Torrent }) {
  const now = useNow();
  const isActive = t.state === "downloading" || t.state === "seeding";
  const nextAnnounce = (() => {
    if (!isActive || !t.last_announced_at || !t.announce_interval) return "—";
    const next = t.last_announced_at + t.announce_interval * 1000;
    return fmtCountdown(next - now);
  })();

  return (
    <div className="grid grid-cols-2 gap-px bg-border">
      <Stat
        icon={<HardDrive className="size-3" strokeWidth={1.75} />}
        label="Size"
        value={fmtBytes(t.size)}
      />
      <Stat
        icon={<Scale className="size-3" strokeWidth={1.75} />}
        label="Ratio"
        value={fmtRatio(t.uploaded, t.size)}
        accent
      />
      <Stat
        icon={<Upload className="size-3" strokeWidth={1.75} />}
        label="Uploaded"
        value={fmtBytes(t.uploaded)}
      />
      <Stat
        icon={<TrendingDown className="size-3" strokeWidth={1.75} />}
        label="Downloaded"
        value={fmtBytes(t.downloaded)}
      />
      <Stat
        icon={<HardDrive className="size-3" strokeWidth={1.75} />}
        label="Left"
        value={fmtBytes(t.left)}
      />
      <Stat
        icon={<Timer className="size-3" strokeWidth={1.75} />}
        label="Next announce"
        value={nextAnnounce}
      />
      <Stat
        icon={<TrendingUp className="size-3" strokeWidth={1.75} />}
        label="Up"
        value={fmtSpeed(t.upload_speed)}
        live={(t.upload_speed ?? 0) > 0}
      />
      <Stat
        icon={<TrendingDown className="size-3" strokeWidth={1.75} />}
        label="Down"
        value={fmtSpeed(t.download_speed)}
        live={(t.download_speed ?? 0) > 0}
      />
      <Stat
        icon={<Users className="size-3" strokeWidth={1.75} />}
        label="Seeders"
        value={t.seeders ?? "—"}
      />
      <Stat
        icon={<Users className="size-3" strokeWidth={1.75} />}
        label="Leechers"
        value={t.leechers ?? "—"}
      />
      <Stat
        icon={<RefreshCw className="size-3" strokeWidth={1.75} />}
        label="Interval"
        value={t.announce_interval ? `${t.announce_interval}s` : "—"}
      />
      <Stat
        icon={<RefreshCw className="size-3" strokeWidth={1.75} />}
        label="Min interval"
        value={t.min_announce_interval ? `${t.min_announce_interval}s` : "—"}
      />
    </div>
  );
}

function Stat({
  icon,
  label,
  value,
  live,
  accent,
}: {
  icon?: React.ReactNode;
  label: string;
  value: React.ReactNode;
  live?: boolean;
  accent?: boolean;
}) {
  return (
    <div className="flex flex-col gap-1.5 bg-background px-4 py-3.5">
      <span className="eyebrow flex items-center gap-1.5 text-muted-foreground">
        {live && (
          <span className="text-success">
            <span className="dot-live" aria-hidden="true" />
          </span>
        )}
        {icon}
        {label}
      </span>
      <span
        className={cn(
          "num text-[15px] font-medium leading-none",
          accent && "text-foreground",
        )}
      >
        {value}
      </span>
    </div>
  );
}

/* ───────────────────────────── trackers list ────────────────────────── */

function TrackersList({ tiers }: { tiers: string[][] | undefined }) {
  const groups = (tiers ?? []).filter((t) => t.length > 0);
  const total = groups.reduce((n, t) => n + t.length, 0);
  return (
    <section className="border-b">
      <header className="flex items-center justify-between px-4 pb-2 pt-4">
        <span className="eyebrow-strong">Trackers</span>
        <span className="num text-[11px] text-muted-foreground">
          {groups.length === 0
            ? "0"
            : `${total} · ${groups.length} ${groups.length === 1 ? "tier" : "tiers"}`}
        </span>
      </header>
      {total === 0 ? (
        <p className="px-4 pb-4 font-mono text-[12px] text-muted-foreground">
          No HTTP trackers
        </p>
      ) : (
        groups.map((tier, ti) => (
          <ul key={`tier-${ti}`} className="divide-y border-y">
            {groups.length > 1 && (
              <li className="bg-muted/30 px-4 py-1 font-mono text-[10px] uppercase tracking-wider text-muted-foreground">
                tier {(ti + 1).toString().padStart(2, "0")}
              </li>
            )}
            {tier.map((url, i) => (
              <li
                key={url}
                className="flex items-center gap-3 px-4 py-2 font-mono text-[12px]"
              >
                <span className="num w-5 shrink-0 text-right text-muted-foreground">
                  {(i + 1).toString().padStart(2, "0")}
                </span>
                <span className="min-w-0 flex-1 truncate" title={url}>
                  {url}
                </span>
              </li>
            ))}
          </ul>
        ))
      )}
    </section>
  );
}

/* ─────────────────────────── announces feed ─────────────────────────── */

function AnnouncesConsole({
  page,
  pageIndex,
  pageSize,
  onPageChange,
  isLoading,
  isFetching,
}: {
  page: AnnouncesPage | null;
  pageIndex: number;
  pageSize: number;
  onPageChange: (next: number) => void;
  isLoading: boolean;
  isFetching: boolean;
}) {
  // Server already returns newest-first; `items[i+1]` is the next-older entry for deltas.
  const items = page?.items ?? [];
  const total = page?.total ?? 0;
  const start = pageIndex * pageSize;
  const end = Math.min(start + items.length, total);
  const hasPrev = pageIndex > 0;
  const hasNext = end < total;
  const totalPages = Math.max(1, Math.ceil(total / pageSize));

  return (
    <section>
      <header className="flex items-center justify-between gap-2 px-4 pb-2 pt-4">
        <span className="eyebrow-strong">Announces</span>
        <span className="num font-mono text-[11px] text-muted-foreground">
          {total}
        </span>
      </header>

      <div className="border-t font-mono text-[11.5px]">
        {isLoading && items.length === 0 ? (
          <p className="px-4 py-8 text-center text-muted-foreground">
            awaiting tracker handshake…
          </p>
        ) : items.length === 0 ? (
          <p className="px-4 py-8 text-center text-muted-foreground">
            no announces recorded
          </p>
        ) : (
          <ul className="divide-y divide-border/50">
            {items.map((a, i) => (
              <AnnounceLine
                key={`${a.announced_at}-${a.tracker_index}-${a.event}`}
                a={a}
                prev={items[i + 1]}
              />
            ))}
          </ul>
        )}
      </div>

      {total > pageSize && (
        <div className="flex items-center justify-between gap-3 border-t px-4 py-2.5 font-mono text-[11px] text-muted-foreground">
          <span className="num">
            {total === 0 ? 0 : start + 1}–{end} of {total}
          </span>
          <div className="flex items-center gap-2">
            <span className="num">
              p {pageIndex + 1}/{totalPages}
              {isFetching && (
                <span className="ml-1.5 text-success/80" aria-hidden="true">
                  •
                </span>
              )}
            </span>
            <button
              type="button"
              onClick={() => onPageChange(pageIndex - 1)}
              disabled={!hasPrev || isFetching}
              className="inline-flex h-6 items-center rounded-sm border border-border/60 px-2 text-[10.5px] uppercase tracking-wider transition-colors enabled:hover:bg-accent disabled:opacity-40"
            >
              Prev
            </button>
            <button
              type="button"
              onClick={() => onPageChange(pageIndex + 1)}
              disabled={!hasNext || isFetching}
              className="inline-flex h-6 items-center rounded-sm border border-border/60 px-2 text-[10.5px] uppercase tracking-wider transition-colors enabled:hover:bg-accent disabled:opacity-40"
            >
              Next
            </button>
          </div>
        </div>
      )}
    </section>
  );
}

function AnnounceLine({ a, prev }: { a: AnnounceTrace; prev?: AnnounceTrace }) {
  const [open, setOpen] = useState(false);

  // Deltas vs previous announce (only for byte fields that grow or shrink)
  const deltaUp = prev
    ? a.request.params.uploaded - prev.request.params.uploaded
    : null;
  const deltaDn = prev
    ? a.request.params.downloaded - prev.request.params.downloaded
    : null;
  const deltaLeft = prev
    ? a.request.params.left - prev.request.params.left
    : null;

  return (
    <li className="bg-card/20">
      {/* ── Summary row (always visible) ── */}
      <button
        type="button"
        className="flex w-full items-center gap-2.5 px-4 py-2.5 text-left transition-colors hover:bg-accent/30"
        onClick={() => setOpen((v) => !v)}
        aria-expanded={open}
      >
        <span
          className={cn(
            "flex size-[18px] shrink-0 items-center justify-center rounded-sm font-mono text-[11px] font-semibold leading-none",
            a.success
              ? "bg-success/15 text-success"
              : "bg-destructive/15 text-destructive",
          )}
          title={a.success ? "ok" : "failed"}
          aria-label={a.success ? "ok" : "failed"}
        >
          {a.success ? "✓" : "✗"}
        </span>

        <span
          className={cn(
            "w-[58px] shrink-0 font-mono text-[10px] font-semibold uppercase tracking-wider",
            a.event === "started" && "text-success",
            a.event === "stopped" && "text-warn",
            a.event === "completed" && "text-signal",
            a.event === "none" && "text-muted-foreground",
          )}
        >
          {a.event === "none" ? "update" : a.event}
        </span>

        <span className="num shrink-0 text-[10.5px] text-muted-foreground">
          tr#{a.tracker_index}
        </span>

        <SizeDeltaChip
          up={a.request.params.uploaded}
          down={a.request.params.downloaded}
          upDelta={deltaUp}
          downDelta={deltaDn}
        />

        <span className="fade-x min-w-0 flex-1 truncate text-[10.5px] text-muted-foreground/70">
          {a.request.url}
        </span>

        <span className="num shrink-0 text-[10.5px] text-muted-foreground">
          {fmtRelativeTime(a.announced_at)}
        </span>

        {open ? (
          <ChevronDown
            className="size-3 shrink-0 text-muted-foreground"
            strokeWidth={2}
          />
        ) : (
          <ChevronRight
            className="size-3 shrink-0 text-muted-foreground"
            strokeWidth={2}
          />
        )}
      </button>

      {/* ── Expanded detail panel ── */}
      {open && (
        <div className="border-t border-border/40 bg-background/40 px-4 pb-4 pt-3 text-[11px]">
          {/* REQUEST */}
          <SectionLabel>Request</SectionLabel>
          <div className="mt-1.5 space-y-0.5">
            <DetailRow label="URL">
              <span
                className="fade-x block truncate text-foreground/80"
                title={a.request.url}
              >
                {a.request.url}
              </span>
            </DetailRow>
            <DetailRow label="Method">
              <span className="uppercase text-foreground/80">
                {a.request.method}
              </span>
            </DetailRow>
            <DetailRow label="Protocol">
              <span className="text-foreground/80">{a.request.protocol}</span>
            </DetailRow>
          </div>

          <div className="my-2.5 h-px bg-border/40" />

          <SectionLabel>Transfer params</SectionLabel>
          <div className="mt-1.5 space-y-0.5">
            <DetailRow label="Uploaded">
              <ByteVal
                bytes={a.request.params.uploaded}
                delta={deltaUp}
                grow="up"
              />
            </DetailRow>
            <DetailRow label="Downloaded">
              <ByteVal
                bytes={a.request.params.downloaded}
                delta={deltaDn}
                grow="up"
              />
            </DetailRow>
            <DetailRow label="Left">
              <ByteVal
                bytes={a.request.params.left}
                delta={deltaLeft}
                grow="down"
              />
            </DetailRow>
            <DetailRow label="Port">
              <span className="num text-foreground/80">
                {a.request.params.port}
              </span>
            </DetailRow>
          </div>

          {a.request.headers.length > 0 && (
            <>
              <div className="my-2.5 h-px bg-border/40" />
              <SectionLabel>Request headers</SectionLabel>
              <div className="mt-1.5 space-y-0.5">
                {a.request.headers.map((h) => (
                  <DetailRow key={h.name} label={h.name}>
                    <span className="break-all text-foreground/70">
                      {h.value}
                    </span>
                  </DetailRow>
                ))}
              </div>
            </>
          )}

          {/* RESPONSE */}
          <div className="my-2.5 h-px bg-border/40" />
          <SectionLabel>Response</SectionLabel>
          <div className="mt-1.5 space-y-0.5">
            <DetailRow label="Status">
              <span
                className={cn(
                  "num font-semibold",
                  a.success ? "text-success" : "text-destructive",
                )}
              >
                {a.response.status > 0
                  ? a.response.status
                  : a.success
                    ? "OK"
                    : "ERR"}
              </span>
            </DetailRow>
          </div>

          <div className="mt-2">
            <SectionLabel>Body</SectionLabel>
            <pre className="mt-1 overflow-x-auto whitespace-pre-wrap break-all rounded-sm border border-border/40 bg-background/60 p-2 font-mono text-[10.5px] leading-snug text-foreground/80">
              {JSON.stringify(a.response.body, null, 2)}
            </pre>
          </div>

          {(a.error_code || a.error_message) && (
            <div className="mt-3 break-words rounded-sm bg-destructive/10 px-2.5 py-2 text-destructive">
              {a.error_code && (
                <span className="mr-1.5 font-semibold">[{a.error_code}]</span>
              )}
              {a.error_message}
            </div>
          )}
        </div>
      )}
    </li>
  );
}

function SectionLabel({ children }: { children: React.ReactNode }) {
  return <div className="eyebrow mb-0.5 text-[9.5px]">{children}</div>;
}

function DetailRow({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div className="grid grid-cols-[120px_minmax(0,1fr)] gap-2">
      <span className="text-muted-foreground/70 truncate">{label}</span>
      <span className="min-w-0 font-mono">{children}</span>
    </div>
  );
}

function ByteVal({
  bytes,
  delta,
  grow,
}: {
  bytes: number;
  delta: number | null;
  grow: "up" | "down";
}) {
  const hasGrowth = delta !== null && Math.abs(delta) > 0;
  const isExpected =
    delta !== null &&
    ((grow === "up" && delta > 0) || (grow === "down" && delta < 0));

  return (
    <span className="inline-flex items-center gap-2">
      <span className="num text-foreground/80">{fmtBytes(bytes)}</span>
      {hasGrowth && (
        <span
          className={cn(
            "inline-flex items-center gap-0.5 text-[10px]",
            isExpected ? "text-success" : "text-warn",
          )}
        >
          {delta > 0 ? (
            <TrendingUp className="size-3" strokeWidth={2} />
          ) : (
            <TrendingDown className="size-3" strokeWidth={2} />
          )}
          <span className="num">
            {delta > 0 ? "+" : ""}
            {fmtBytes(Math.abs(delta))}
          </span>
        </span>
      )}
    </span>
  );
}

/* ─────────────────────────── size + delta chip ─────────────────────── */

function SizeDeltaChip({
  up,
  down,
  upDelta,
  downDelta,
}: {
  up: number;
  down: number;
  upDelta: number | null;
  downDelta: number | null;
}) {
  const sign = (n: number) => (n > 0 ? "+" : "−");
  if (up === 0 && down === 0) return null;
  return (
    <span className="hidden shrink-0 items-center gap-2.5 font-mono text-[10.5px] tabular-nums sm:inline-flex">
      {up > 0 && (
        <span
          className="inline-flex items-baseline gap-1"
          title={`uploaded ${fmtBytes(up)}${
            upDelta && upDelta !== 0
              ? ` (${sign(upDelta)}${fmtBytes(Math.abs(upDelta))})`
              : ""
          }`}
        >
          <span className="text-foreground/80">↑{fmtBytes(up)}</span>
          {upDelta !== null && upDelta !== 0 && (
            <span className={cn(upDelta > 0 ? "text-success" : "text-warn")}>
              {sign(upDelta)}
              {fmtBytes(Math.abs(upDelta))}
            </span>
          )}
        </span>
      )}
      {down > 0 && (
        <span
          className="inline-flex items-baseline gap-1"
          title={`downloaded ${fmtBytes(down)}${
            downDelta && downDelta !== 0
              ? ` (${sign(downDelta)}${fmtBytes(Math.abs(downDelta))})`
              : ""
          }`}
        >
          <span className="text-foreground/80">↓{fmtBytes(down)}</span>
          {downDelta !== null && downDelta !== 0 && (
            <span className={cn(downDelta > 0 ? "text-signal" : "text-warn")}>
              {sign(downDelta)}
              {fmtBytes(Math.abs(downDelta))}
            </span>
          )}
        </span>
      )}
    </span>
  );
}

/* ───────────────────────────── copy button ──────────────────────────── */

function CopyButton({ value, label }: { value: string; label: string }) {
  const [copied, setCopied] = useState(false);
  useEffect(() => {
    if (!copied) return;
    const t = window.setTimeout(() => setCopied(false), 1200);
    return () => window.clearTimeout(t);
  }, [copied]);
  return (
    <button
      type="button"
      className="inline-flex size-5 shrink-0 items-center justify-center rounded-sm border border-border/60 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
      onClick={async (e) => {
        e.stopPropagation();
        try {
          await navigator.clipboard.writeText(value);
          setCopied(true);
          toast.success(`Copied ${label}`);
        } catch {
          toast.error("Copy failed");
        }
      }}
      aria-label={`Copy ${label}`}
    >
      {copied ? (
        <Check className="size-3" strokeWidth={2.25} />
      ) : (
        <Copy className="size-3" strokeWidth={1.75} />
      )}
    </button>
  );
}
