import { createFileRoute, useNavigate } from "@tanstack/react-router";
import {
  Activity,
  ChevronDown,
  ChevronUp,
  Clock,
  Inbox,
  TrendingDown,
  TrendingUp,
} from "lucide-react";
import { useMemo, useState } from "react";
import { z } from "zod";

import { AddTorrentDialog } from "@/components/add-torrent-dialog";
import { PresetChipStrip } from "@/components/preset-chip-strip";
import { PresetPill } from "@/components/preset-pill";
import { TorrentActions } from "@/components/torrent-actions";
import { TorrentDetailSheet } from "@/components/torrent-detail-sheet";
import { TorrentStatusBadge } from "@/components/torrent-status-badge";
import { useNow } from "@/hooks/use-now";
import {
  fmtBytes,
  fmtCountdown,
  fmtRatio,
  fmtSpeed,
  shortHash,
} from "@/lib/format";
import { usePresetSelection } from "@/lib/preset-context";
import { usePresets, useStats, useTorrents } from "@/lib/queries";
import type { Preset, Torrent } from "@/lib/types";
import { cn } from "@/lib/utils";

const searchSchema = z.object({
  selected: z.string().optional().catch(undefined),
});

export const Route = createFileRoute("/_authed/")({
  validateSearch: searchSchema,
  component: TorrentsPage,
});

type SortKey =
  | "name"
  | "state"
  | "size"
  | "ratio"
  | "upload_speed"
  | "download_speed"
  | "swarm"
  | "next";
type SortDir = "asc" | "desc";

const STATE_RANK: Record<Torrent["state"], number> = {
  downloading: 0,
  seeding: 1,
  queued: 2,
  stopped: 3,
};

function nextAnnounceMs(t: Torrent): number {
  if (t.state !== "downloading" && t.state !== "seeding") return Infinity;
  if (!t.last_announced_at || !t.announce_interval) return Infinity;
  return t.last_announced_at + t.announce_interval * 1000;
}

function compareBy(a: Torrent, b: Torrent, key: SortKey): number {
  switch (key) {
    case "name":
      return a.name.localeCompare(b.name);
    case "state":
      return STATE_RANK[a.state] - STATE_RANK[b.state];
    case "size":
      return (a.size ?? 0) - (b.size ?? 0);
    case "ratio": {
      const ra = (a.uploaded ?? 0) / Math.max(1, a.size ?? 1);
      const rb = (b.uploaded ?? 0) / Math.max(1, b.size ?? 1);
      return ra - rb;
    }
    case "upload_speed":
      return (a.upload_speed ?? 0) - (b.upload_speed ?? 0);
    case "download_speed":
      return (a.download_speed ?? 0) - (b.download_speed ?? 0);
    case "swarm":
      return (a.seeders ?? 0) - (b.seeders ?? 0);
    case "next":
      return nextAnnounceMs(a) - nextAnnounceMs(b);
  }
}

function sortTorrents(rows: Torrent[], key: SortKey, dir: SortDir): Torrent[] {
  const sign = dir === "asc" ? 1 : -1;
  const out = [...rows];
  out.sort((a, b) => {
    const primary = sign * compareBy(a, b, key);
    if (primary !== 0) return primary;
    return a.queue_position - b.queue_position;
  });
  return out;
}

function TorrentsPage() {
  const navigate = useNavigate();
  const { selected } = Route.useSearch();
  const { activeId } = usePresetSelection();
  const { data: presets } = usePresets();
  // Pull a wide page so list-level sort works across all rows in the active scope.
  const { data: page, isLoading } = useTorrents({
    presetId: activeId,
    offset: 0,
    limit: 200,
  });
  const data = page?.items;
  const stats = useStats(activeId);

  // Active preset (when filtering): drive scoped policy hints and pill colors.
  const activePreset =
    activeId === "all"
      ? null
      : ((presets ?? []).find((p) => p.id === activeId) ?? null);
  const ratioTarget = activePreset?.policy.upload_ratio_target;
  const pauseOnZL = activePreset?.policy.pause_torrent_with_zero_leechers;
  const slotCap = activePreset?.policy.max_active_torrents;
  const ratioLabel =
    activeId === "all"
      ? "across all presets"
      : ratioTarget == null
        ? null
        : ratioTarget <= 0
          ? "target off"
          : `target ${ratioTarget}×`;
  const pauseLabel =
    activeId === "all"
      ? null
      : pauseOnZL == null
        ? null
        : pauseOnZL
          ? "0L pause on"
          : "0L pause off";
  const downloading = (data ?? []).filter((t) => t.state === "downloading");
  const totalLeft = downloading.reduce((sum, t) => sum + (t.left ?? 0), 0);
  const downloadSub =
    downloading.length === 0
      ? null
      : `${downloading.length} active · ${fmtBytes(totalLeft)} left`;
  const presetById = useMemo(() => {
    const m = new Map<string, Preset>();
    for (const p of presets ?? []) m.set(p.id, p);
    return m;
  }, [presets]);
  const totals = useMemo(() => {
    const t: Record<string, number> = {
      all: stats.data?.tracked_metainfo_torrents ?? 0,
    };
    for (const p of presets ?? []) {
      t[p.id] = p.rollup?.torrent_count ?? 0;
    }
    return t;
  }, [presets, stats.data]);
  const [sort, setSort] = useState<{
    key: SortKey;
    dir: SortDir;
    userSet: boolean;
  }>({ key: "state", dir: "asc", userSet: false });
  const torrents = useMemo(
    () => sortTorrents(data ?? [], sort.key, sort.dir),
    [data, sort],
  );
  const onSort = (key: SortKey) => {
    setSort((s) =>
      s.key === key
        ? { key, dir: s.dir === "asc" ? "desc" : "asc", userSet: true }
        : { key, dir: "asc", userSet: true },
    );
  };

  const now = useNow();
  const open = (infoHash: string | undefined | null) => {
    if (!infoHash) return;
    navigate({ to: "/", search: { selected: infoHash } });
  };
  const close = () => navigate({ to: "/", search: {} });

  return (
    <div className="px-3 pb-12 pt-4 md:px-6 md:pt-6">
      {/* ── Page header ── */}
      <header className="mb-3 flex items-end justify-between gap-4 md:mb-5">
        <div>
          <div className="eyebrow mb-1.5">Operations · Torrents</div>
          <h1 className="flex items-center gap-2.5 text-[22px] font-semibold leading-tight tracking-tight md:text-[28px]">
            {activePreset ? (
              <>
                <span
                  aria-hidden="true"
                  className="inline-block size-2.5 shrink-0 rounded-full"
                  style={{ background: activePreset.color }}
                />
                <span>{activePreset.name}</span>
              </>
            ) : (
              "Tracker pulse"
            )}
          </h1>
        </div>
        <AddTorrentDialog />
      </header>

      {/* ── Preset chip strip ── */}
      <div className="mb-4 md:mb-5">
        <PresetChipStrip
          totals={totals}
          onCreate={() =>
            navigate({ to: "/config", search: { new: "1" } as never })
          }
        />
      </div>

      {/* ── Hero summary ── */}
      <section className="mb-6 grid grid-cols-2 gap-px overflow-hidden rounded-md border bg-border md:grid-cols-4">
        <HeroStat
          icon={<Activity className="size-3" strokeWidth={1.75} />}
          label="Active"
          value={stats.data ? `${stats.data.active_torrents}` : "—"}
          valueSuffix={
            slotCap != null
              ? `/ ${slotCap}`
              : stats.data
                ? `/ ${stats.data.max_active_torrents}`
                : undefined
          }
          sub={pauseLabel ?? undefined}
        />
        <HeroStat
          icon={<Clock className="size-3" strokeWidth={1.75} />}
          label="Waiting"
          value={stats.data ? `${stats.data.waiting_torrents}` : "—"}
          sub={
            stats.data
              ? `of ${stats.data.tracked_metainfo_torrents} total`
              : undefined
          }
        />
        <HeroStat
          icon={<TrendingUp className="size-3" strokeWidth={1.75} />}
          label="Aggregate up"
          value={fmtSpeed(stats.data?.upload_speed)}
          sub={ratioLabel ?? undefined}
        />
        <HeroStat
          icon={<TrendingDown className="size-3" strokeWidth={1.75} />}
          label="Aggregate down"
          value={fmtSpeed(stats.data?.download_speed)}
          sub={downloadSub ?? undefined}
        />
      </section>

      {/* ── List ── */}
      <section>
        <header className="mb-2 flex items-center justify-between">
          <span className="eyebrow-strong">Torrents</span>
          <span className="num text-[11px] text-muted-foreground">
            {isLoading
              ? "—"
              : `${torrents.length.toString().padStart(2, "0")} ROWS`}
          </span>
        </header>

        {torrents.length === 0 && !isLoading ? (
          <EmptyState />
        ) : (
          <>
            {/* Mobile cards */}
            <ul className="space-y-1.5 md:hidden">
              {torrents.map((t) => (
                <TorrentCard
                  key={t.id}
                  t={t}
                  now={now}
                  onOpen={open}
                  preset={presetById.get(t.preset_id) ?? null}
                  showPresetPill={activeId === "all"}
                />
              ))}
            </ul>

            {/* Desktop table */}
            <div className="hidden overflow-hidden rounded-md border bg-card md:block">
              <table className="w-full text-[12.5px]">
                <thead>
                  <tr className="border-b text-left text-muted-foreground">
                    <SortableTh sortKey="name" sort={sort} onSort={onSort}>
                      Name
                    </SortableTh>
                    {activeId === "all" && (
                      <Th className="w-[150px]">Preset</Th>
                    )}
                    <SortableTh
                      className="w-[120px]"
                      sortKey="state"
                      sort={sort}
                      onSort={onSort}
                    >
                      Status
                    </SortableTh>
                    <SortableTh
                      className="w-[88px]"
                      align="right"
                      sortKey="size"
                      sort={sort}
                      onSort={onSort}
                    >
                      Size
                    </SortableTh>
                    <SortableTh
                      className="w-[72px]"
                      align="right"
                      sortKey="ratio"
                      sort={sort}
                      onSort={onSort}
                    >
                      Ratio
                    </SortableTh>
                    <SortableTh
                      className="w-[88px]"
                      align="right"
                      sortKey="upload_speed"
                      sort={sort}
                      onSort={onSort}
                    >
                      ↑
                    </SortableTh>
                    <SortableTh
                      className="w-[88px]"
                      align="right"
                      sortKey="download_speed"
                      sort={sort}
                      onSort={onSort}
                    >
                      ↓
                    </SortableTh>
                    <SortableTh
                      className="w-[80px]"
                      align="right"
                      sortKey="swarm"
                      sort={sort}
                      onSort={onSort}
                    >
                      S / L
                    </SortableTh>
                    <SortableTh
                      className="w-[80px]"
                      align="right"
                      sortKey="next"
                      sort={sort}
                      onSort={onSort}
                    >
                      Next
                    </SortableTh>
                    <Th className="w-[44px]" />
                  </tr>
                </thead>
                <tbody>
                  {torrents.map((t) => (
                    <TorrentRow
                      key={t.id}
                      t={t}
                      now={now}
                      onOpen={open}
                      preset={presetById.get(t.preset_id) ?? null}
                      showPresetCol={activeId === "all"}
                    />
                  ))}
                </tbody>
              </table>
            </div>
          </>
        )}
      </section>

      <TorrentDetailSheet infoHash={selected ?? null} onClose={close} />
    </div>
  );
}

/* ───────────────────────── HERO STAT ───────────────────────── */

function ratioColorClass(
  uploaded?: number | null,
  size?: number | null,
): string {
  const u = uploaded ?? 0;
  const s = size ?? 0;
  if (s === 0 || u === 0) return "text-muted-foreground/60";
  const r = u / s;
  if (r < 0.01) return "text-muted-foreground/60";
  if (r < 0.5) return "text-destructive/65";
  if (r < 1) return "text-amber-500/75";
  if (r < 2) return "text-success/75";
  return "text-success/85";
}

function HeroStat({
  icon,
  label,
  value,
  valueSuffix,
  sub,
}: {
  icon: React.ReactNode;
  label: string;
  value: string;
  /** Smaller, muted text rendered immediately after the main value (e.g., "of 5"). */
  valueSuffix?: string;
  sub?: string;
}) {
  return (
    <div className="flex flex-col gap-2 bg-background p-4 md:p-5">
      <div className="flex items-center gap-1.5 text-muted-foreground">
        {icon}
        <span className="eyebrow">{label}</span>
      </div>
      <div className="num flex items-baseline gap-1.5 text-[24px] font-semibold leading-none tracking-tight md:text-[28px]">
        <span>{value}</span>
        {valueSuffix && (
          <span className="text-[14px] font-medium text-muted-foreground/70 md:text-[15px]">
            {valueSuffix}
          </span>
        )}
      </div>
      {sub && (
        <div className="font-mono text-[10.5px] uppercase tracking-wider text-muted-foreground/80">
          {sub}
        </div>
      )}
    </div>
  );
}

/* ───────────────────────── TABLE ───────────────────────── */

function Th({
  children,
  align = "left",
  className,
}: {
  children?: React.ReactNode;
  align?: "left" | "right";
  className?: string;
}) {
  return (
    <th
      className={cn(
        "h-9 px-3 align-middle font-mono text-[10px] font-medium uppercase tracking-[0.14em]",
        align === "right" && "text-right",
        className,
      )}
    >
      {children}
    </th>
  );
}

function SortableTh({
  children,
  align = "left",
  className,
  sortKey,
  sort,
  onSort,
}: {
  children?: React.ReactNode;
  align?: "left" | "right";
  className?: string;
  sortKey: SortKey;
  sort: { key: SortKey; dir: SortDir; userSet: boolean };
  onSort: (k: SortKey) => void;
}) {
  const showArrow = sort.key === sortKey && sort.userSet;
  return (
    <th
      className={cn(
        "h-9 px-3 align-middle font-mono text-[10px] font-medium uppercase tracking-[0.14em]",
        align === "right" && "text-right",
        className,
      )}
    >
      <button
        type="button"
        onClick={() => onSort(sortKey)}
        className={cn(
          "inline-flex items-center gap-1 transition-colors hover:text-foreground",
          align === "right" && "flex-row-reverse",
        )}
      >
        <span>{children}</span>
        {showArrow &&
          (sort.dir === "asc" ? (
            <ChevronUp className="size-3" strokeWidth={2.25} />
          ) : (
            <ChevronDown className="size-3" strokeWidth={2.25} />
          ))}
      </button>
    </th>
  );
}

function Td({
  children,
  align = "left",
  className,
  nowrap,
}: {
  children?: React.ReactNode;
  align?: "left" | "right";
  className?: string;
  nowrap?: boolean;
}) {
  return (
    <td
      className={cn(
        "px-3 py-2.5 align-middle",
        align === "right" && "text-right",
        nowrap && "whitespace-nowrap",
        className,
      )}
    >
      {children}
    </td>
  );
}

function TorrentRow({
  t,
  now,
  onOpen,
  preset,
  showPresetCol,
}: {
  t: Torrent;
  now: number;
  onOpen: (h: string | null | undefined) => void;
  preset: Preset | null;
  showPresetCol: boolean;
}) {
  const isActive = t.state === "downloading" || t.state === "seeding";
  const nextCountdown = (() => {
    if (!isActive || !t.last_announced_at || !t.announce_interval) return "—";
    return fmtCountdown(t.last_announced_at + t.announce_interval * 1000 - now);
  })();
  const countdownLive = nextCountdown !== "—";

  return (
    <tr
      className="cursor-pointer border-b border-border/60 transition-colors last:border-b-0 hover:bg-accent/40"
      onClick={() => onOpen(t.info_hash)}
    >
      <Td className="max-w-0">
        <div className="flex items-center gap-2">
          {!showPresetCol && preset && (
            <span
              aria-hidden="true"
              className="size-1.5 shrink-0 rounded-full"
              style={{ background: preset.color }}
            />
          )}
          <span className="min-w-0 flex-1 truncate font-medium leading-tight">
            {t.name}
          </span>
        </div>
        <div className="num mt-0.5 truncate text-[11px] text-muted-foreground">
          {shortHash(t.info_hash)}
        </div>
      </Td>
      {showPresetCol && (
        <Td nowrap>
          {preset ? (
            <PresetPill color={preset.color} name={preset.name} />
          ) : (
            <span className="font-mono text-[11px] text-muted-foreground/50">
              —
            </span>
          )}
        </Td>
      )}
      <Td nowrap>
        <TorrentStatusBadge t={t} />
      </Td>
      <Td align="right" nowrap>
        <span className="num text-foreground/80">{fmtBytes(t.size)}</span>
      </Td>
      <Td align="right" nowrap>
        <span className={cn("num", ratioColorClass(t.uploaded, t.size))}>
          {fmtRatio(t.uploaded, t.size)}
        </span>
      </Td>
      <Td align="right" nowrap>
        <span className="num text-foreground/80">
          {fmtSpeed(t.upload_speed)}
        </span>
      </Td>
      <Td align="right" nowrap>
        <span className="num text-foreground/80">
          {fmtSpeed(t.download_speed)}
        </span>
      </Td>
      <Td align="right" nowrap>
        <span className="num text-foreground/80">
          {t.seeders ?? "—"} / {t.leechers ?? "—"}
        </span>
      </Td>
      <Td align="right" nowrap>
        <span
          className={cn(
            "num tabular-nums",
            countdownLive ? "text-foreground/80" : "text-muted-foreground",
          )}
        >
          {nextCountdown}
        </span>
      </Td>
      <Td>
        {/** biome-ignore lint/a11y/noStaticElementInteractions: action wrapper inside clickable row */}
        <div
          onClick={(e) => e.stopPropagation()}
          onKeyDown={(e) => e.stopPropagation()}
        >
          <TorrentActions t={t} />
        </div>
      </Td>
    </tr>
  );
}

/* ───────────────────────── MOBILE CARD ───────────────────────── */

function TorrentCard({
  t,
  now,
  onOpen,
  preset,
  showPresetPill,
}: {
  t: Torrent;
  now: number;
  onOpen: (h: string | null | undefined) => void;
  preset: Preset | null;
  showPresetPill: boolean;
}) {
  const isActive = t.state === "downloading" || t.state === "seeding";
  const nextCountdown = (() => {
    if (!isActive || !t.last_announced_at || !t.announce_interval) return "—";
    return fmtCountdown(t.last_announced_at + t.announce_interval * 1000 - now);
  })();
  const railColor = stateRailColor(t);
  const upBps = t.upload_speed ?? 0;
  const dnBps = t.download_speed ?? 0;

  return (
    <li>
      <button
        type="button"
        onClick={() => onOpen(t.info_hash)}
        className="group relative block w-full overflow-hidden rounded-md border bg-card pl-[10px] pr-2.5 py-2.5 text-left transition-colors hover:bg-accent/40 active:bg-accent/60"
      >
        {/* Left state rail. Full height, 3px wide, color encodes state. */}
        <span
          aria-hidden="true"
          className="pointer-events-none absolute inset-y-0 left-0 w-[3px]"
          style={{ background: railColor }}
        />

        {/* Line 1 — name + actions */}
        <div className="flex items-center justify-between gap-2">
          <div className="min-w-0 flex-1 truncate text-[13.5px] font-medium leading-tight">
            {t.name}
          </div>
          {/** biome-ignore lint/a11y/noStaticElementInteractions: action wrapper inside clickable card */}
          <div
            className="-mr-1 shrink-0"
            onClick={(e) => e.stopPropagation()}
            onKeyDown={(e) => e.stopPropagation()}
          >
            <TorrentActions t={t} />
          </div>
        </div>

        {/* Line 2 — single horizontal metric strip with `·` separators. */}
        <div className="mt-1 flex flex-wrap items-center gap-x-2 gap-y-0.5 num text-[11px] text-muted-foreground">
          <TorrentStatusBadge t={t} />
          {showPresetPill && preset && (
            <>
              <Sep />
              <PresetPill color={preset.color} name={preset.name} />
            </>
          )}
          <Sep />
          <span className="text-foreground/80">{fmtBytes(t.size)}</span>
          <Sep />
          <span
            className={cn("font-medium", ratioColorClass(t.uploaded, t.size))}
          >
            {fmtRatio(t.uploaded, t.size)}×
          </span>
          <Sep />
          <span
            className={cn(
              "inline-flex items-center gap-0.5",
              upBps > 0 && "text-success",
            )}
          >
            <span aria-hidden>↑</span>
            <span>{compactSpeed(upBps)}</span>
          </span>
          <span
            className={cn(
              "inline-flex items-center gap-0.5",
              dnBps > 0 && "text-signal",
            )}
          >
            <span aria-hidden>↓</span>
            <span>{compactSpeed(dnBps)}</span>
          </span>
          <Sep />
          <span className="tabular-nums">
            {t.seeders ?? "—"}
            <span className="text-muted-foreground/50">/</span>
            {t.leechers ?? "—"}
          </span>
          <Sep />
          <span className="tabular-nums">{nextCountdown}</span>
        </div>
      </button>
    </li>
  );
}

function Sep() {
  return (
    <span aria-hidden="true" className="text-muted-foreground/40">
      ·
    </span>
  );
}

/** Map torrent state → CSS-var color used by the left rail. */
function stateRailColor(t: Torrent): string {
  switch (t.state) {
    case "seeding":
      return "var(--success)";
    case "downloading":
      return "var(--signal)";
    case "queued":
      return "var(--muted-foreground)";
    case "stopped":
      switch (t.reason) {
        case "tracker_failed":
          return "var(--destructive)";
        case "upload_ratio":
        case "no_leechers":
          return "var(--warn)";
        default:
          return "var(--muted-foreground)";
      }
  }
}

/** Compact speed: drop "/s" suffix and zero-pad to keep strip width stable. */
function compactSpeed(bps: number): string {
  if (!bps) return "0";
  return fmtSpeed(bps).replace(/\s?\/s$/, "");
}

/* ───────────────────────── EMPTY ───────────────────────── */

function EmptyState() {
  return (
    <div className="rounded-md border border-dashed bg-card/30 p-10 text-center">
      <Inbox
        className="mx-auto mb-3 size-8 text-muted-foreground/60"
        strokeWidth={1.25}
      />
      <div className="text-[13px] font-medium">No torrents tracked</div>
      <p className="mt-1 font-mono text-[11px] text-muted-foreground">
        add a `.torrent` file to spawn the announce loop
      </p>
    </div>
  );
}
