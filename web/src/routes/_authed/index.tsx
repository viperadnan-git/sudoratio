import { createFileRoute, useNavigate } from "@tanstack/react-router";
import {
  Activity,
  ChevronDown,
  ChevronUp,
  Clock,
  HardDrive,
  Inbox,
  Scale,
  Timer,
  TrendingDown,
  TrendingUp,
  Users,
} from "lucide-react";
import { useMemo, useState } from "react";
import { z } from "zod";

import { AddTorrentDialog } from "@/components/add-torrent-dialog";
import { PresetChipStrip } from "@/components/preset-chip-strip";
import { PresetPill } from "@/components/preset-pill";
import {
  TorrentActions,
  TorrentActionsKebab,
  TorrentRowContextMenu,
  useTorrentMenu,
} from "@/components/torrent-actions";
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
    const t: Record<string, number> = { all: 0 };
    for (const p of presets ?? []) {
      const n = p.rollup?.torrent_count ?? 0;
      t[p.id] = n;
      t.all += n;
    }
    return t;
  }, [presets]);
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
              <table className="w-full table-fixed text-[12.5px]">
                <thead>
                  <tr className="border-b text-left text-muted-foreground">
                    <SortableTh sortKey="name" sort={sort} onSort={onSort}>
                      Name
                    </SortableTh>
                    <SortableTh
                      className="w-[104px] lg:w-[120px]"
                      sortKey="state"
                      sort={sort}
                      onSort={onSort}
                    >
                      Status
                    </SortableTh>
                    <SortableTh
                      className="w-[80px] lg:w-[88px]"
                      align="right"
                      sortKey="size"
                      sort={sort}
                      onSort={onSort}
                    >
                      Size
                    </SortableTh>
                    <SortableTh
                      className="w-[60px] lg:w-[72px]"
                      align="right"
                      sortKey="ratio"
                      sort={sort}
                      onSort={onSort}
                    >
                      Ratio
                    </SortableTh>
                    <SortableTh
                      className="w-[92px] lg:w-[96px]"
                      align="right"
                      sortKey="upload_speed"
                      sort={sort}
                      onSort={onSort}
                    >
                      ↑
                    </SortableTh>
                    <SortableTh
                      className="w-[92px] lg:w-[96px]"
                      align="right"
                      sortKey="download_speed"
                      sort={sort}
                      onSort={onSort}
                    >
                      ↓
                    </SortableTh>
                    <SortableTh
                      className="hidden w-[80px] lg:table-cell"
                      align="right"
                      sortKey="swarm"
                      sort={sort}
                      onSort={onSort}
                    >
                      S / L
                    </SortableTh>
                    <SortableTh
                      className="hidden w-[80px] lg:table-cell"
                      align="right"
                      sortKey="next"
                      sort={sort}
                      onSort={onSort}
                    >
                      Next
                    </SortableTh>
                    <Th className="w-[40px] lg:w-[44px]" />
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
    <div className="flex flex-col gap-1.5 bg-background p-3 md:gap-2 md:p-5">
      <div className="flex items-center gap-1.5 text-muted-foreground">
        {icon}
        <span className="eyebrow">{label}</span>
      </div>
      <div className="num flex items-baseline gap-1.5 text-[18px] font-semibold leading-none tracking-tight md:text-[22px] lg:text-[28px]">
        <span>{value}</span>
        {valueSuffix && (
          <span className="text-[11px] font-medium text-muted-foreground/70 md:text-[12px] lg:text-[15px]">
            {valueSuffix}
          </span>
        )}
      </div>
      {sub && (
        <div className="font-mono text-[10px] uppercase tabular-nums tracking-wider text-muted-foreground/80 md:text-[10.5px]">
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
}: {
  t: Torrent;
  now: number;
  onOpen: (h: string | null | undefined) => void;
  preset: Preset | null;
}) {
  const isActive = t.state === "downloading" || t.state === "seeding";
  const nextCountdown = (() => {
    if (!isActive || !t.last_announced_at || !t.announce_interval) return "—";
    return fmtCountdown(t.last_announced_at + t.announce_interval * 1000 - now);
  })();
  const countdownLive = nextCountdown !== "—";
  const menu = useTorrentMenu(t);

  const row = (
    <tr
      className="cursor-pointer border-b border-border/60 transition-colors last:border-b-0 hover:bg-accent/40 data-[state=open]:bg-accent/50"
      onClick={() => onOpen(t.info_hash)}
    >
      <Td>
        <div className="truncate font-medium leading-tight" title={t.name}>
          {t.name}
        </div>
        <div className="mt-0.5 flex min-w-0 items-center gap-1.5">
          {preset && (
            <PresetPill
              color={preset.color}
              name={preset.name}
              className="shrink-0"
            />
          )}
          <span className="num truncate text-[11px] text-muted-foreground">
            {shortHash(t.info_hash)}
          </span>
        </div>
      </Td>
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
      <Td align="right" nowrap className="hidden lg:table-cell">
        <span className="num text-foreground/80">
          {t.seeders ?? "—"} / {t.leechers ?? "—"}
        </span>
      </Td>
      <Td align="right" nowrap className="hidden lg:table-cell">
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
          {menu && <TorrentActionsKebab menu={menu} />}
        </div>
      </Td>
    </tr>
  );

  if (!menu) return row;
  return (
    <>
      <TorrentRowContextMenu menu={menu}>{row}</TorrentRowContextMenu>
      {menu.dialogs}
    </>
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
  const isDownloading = t.state === "downloading";
  const nextCountdown = (() => {
    if (!isActive || !t.last_announced_at || !t.announce_interval) return "—";
    return fmtCountdown(t.last_announced_at + t.announce_interval * 1000 - now);
  })();
  const upBps = t.upload_speed ?? 0;
  const dnBps = t.download_speed ?? 0;
  const pct = (() => {
    if (!isDownloading) return null;
    const size = t.size ?? 0;
    if (size <= 0) return null;
    const left = t.left ?? size;
    return Math.max(0, Math.min(100, (100 * (size - left)) / size));
  })();

  return (
    <li>
      {/** biome-ignore lint/a11y/useSemanticElements: real <button> would nest the actions trigger; div role=button keeps HTML valid */}
      <div
        role="button"
        tabIndex={0}
        onClick={() => onOpen(t.info_hash)}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            onOpen(t.info_hash);
          }
        }}
        className="group block w-full cursor-pointer overflow-hidden rounded-md border bg-card text-left transition-colors hover:bg-accent/30 active:bg-accent/50 focus-visible:outline-2 focus-visible:outline-foreground/30"
      >
        {/* Hero — name + actions */}
        <div className="flex items-start justify-between gap-2 px-3 pt-3">
          <div className="min-w-0 flex-1">
            <div className="truncate text-[14px] font-semibold leading-tight">
              {t.name}
            </div>
            <div className="mt-1.5 flex flex-wrap items-center gap-x-2 gap-y-1 num text-[10.5px]">
              <TorrentStatusBadge t={t} />
              {showPresetPill && preset && (
                <>
                  <Dot />
                  <PresetPill color={preset.color} name={preset.name} />
                </>
              )}
              <Dot />
              <span className="inline-flex items-center gap-1 text-muted-foreground">
                <HardDrive className="size-3" strokeWidth={1.75} />
                <span className="text-foreground/80">{fmtBytes(t.size)}</span>
              </span>
              <Dot />
              <span
                className={cn(
                  "inline-flex items-center gap-1 font-medium",
                  ratioColorClass(t.uploaded, t.size),
                )}
              >
                <Scale className="size-3" strokeWidth={1.75} />
                <span>{fmtRatio(t.uploaded, t.size)}×</span>
              </span>
            </div>
          </div>
          {/** biome-ignore lint/a11y/noStaticElementInteractions: action wrapper inside clickable card */}
          <div
            className="-mr-1 -mt-0.5 shrink-0"
            onClick={(e) => e.stopPropagation()}
            onKeyDown={(e) => e.stopPropagation()}
          >
            <TorrentActions t={t} />
          </div>
        </div>

        {/* Progress bar — downloading only. */}
        {pct != null && (
          <div className="flex items-center gap-2 px-3 pt-2.5">
            <div className="h-1 flex-1 overflow-hidden rounded-full bg-foreground/10">
              <div
                className="h-full rounded-full bg-signal transition-[width] duration-500 ease-out"
                style={{ width: `${pct}%` }}
              />
            </div>
            <span className="num shrink-0 text-[10.5px] tabular-nums text-muted-foreground">
              {pct.toFixed(0)}%
            </span>
          </div>
        )}

        {/* Metric strip — 4 equal cells, hairline divider above. */}
        <div className="mt-2.5 grid grid-cols-4 gap-x-2 border-t border-border/50 px-3 py-2 num text-[11px] text-muted-foreground">
          <span
            className={cn(
              "inline-flex min-w-0 items-center gap-1.5",
              upBps > 0 && "text-success",
            )}
          >
            <TrendingUp
              className={cn(
                "size-3 shrink-0",
                upBps > 0 ? "text-current" : "text-muted-foreground/70",
              )}
              strokeWidth={1.75}
            />
            <span className="truncate">{compactSpeed(upBps)}</span>
          </span>
          <span
            className={cn(
              "inline-flex min-w-0 items-center gap-1.5",
              dnBps > 0 && "text-signal",
            )}
          >
            <TrendingDown
              className={cn(
                "size-3 shrink-0",
                dnBps > 0 ? "text-current" : "text-muted-foreground/70",
              )}
              strokeWidth={1.75}
            />
            <span className="truncate">{compactSpeed(dnBps)}</span>
          </span>
          <span className="inline-flex min-w-0 items-center gap-1.5 tabular-nums">
            <Users
              className="size-3 shrink-0 text-muted-foreground/70"
              strokeWidth={1.75}
            />
            <span className="truncate">
              {t.seeders ?? "—"}
              <span className="text-muted-foreground/50">/</span>
              {t.leechers ?? "—"}
            </span>
          </span>
          <span className="inline-flex min-w-0 items-center gap-1.5 tabular-nums">
            <Timer
              className="size-3 shrink-0 text-muted-foreground/70"
              strokeWidth={1.75}
            />
            <span className="truncate">{nextCountdown}</span>
          </span>
        </div>
      </div>
    </li>
  );
}

function Dot() {
  return (
    <span aria-hidden="true" className="text-muted-foreground/40">
      ·
    </span>
  );
}

/** Speed in KB/s with thousands separator. */
function compactSpeed(bps: number): string {
  if (!bps) return "0 KB/s";
  return `${Math.round(bps / 1024).toLocaleString()} KB/s`;
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
