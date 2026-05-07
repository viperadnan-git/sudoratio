import { useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import {
  AlertTriangle,
  CheckCircle2,
  Cloud,
  Gauge,
  Globe2,
  Loader2,
  type LucideIcon,
  RotateCcw,
  Save,
  Wifi,
  X,
  XCircle,
} from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";

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
import { Input } from "@/components/ui/input";
import {
  fetchConfigDefaults,
  useCheckConnectivity,
  useConfig,
  useUpdateConfig,
} from "@/lib/queries";
import type { ConfigBody, ConfigUpdate, ConnectivityFamily } from "@/lib/types";
import { cn } from "@/lib/utils";

export const Route = createFileRoute("/_authed/config")({
  component: ConfigPage,
});

function ConfigPage() {
  const cfg = useConfig();
  const update = useUpdateConfig();
  const qc = useQueryClient();
  const [draft, setDraft] = useState<ConfigBody | null>(null);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [resetting, setResetting] = useState(false);
  const dirty =
    cfg.data && draft && Object.keys(diff(cfg.data, draft)).length > 0;

  const onReset = async () => {
    setResetting(true);
    try {
      const defaults = await fetchConfigDefaults(qc);
      setDraft(defaults);
      toast.message("Defaults loaded · review and click Save");
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "fetch defaults failed");
    } finally {
      setResetting(false);
    }
  };

  useEffect(() => {
    if (cfg.data && !draft) setDraft(cfg.data);
  }, [cfg.data, draft]);

  if (!draft) {
    return (
      <div className="px-3 py-6 font-mono text-[12px] text-muted-foreground md:px-6">
        {cfg.isLoading ? "› loading config…" : "› no config available"}
      </div>
    );
  }

  const onReview = (e: React.FormEvent) => {
    e.preventDefault();
    if (!cfg.data) return;
    const patch = diff(cfg.data, draft);
    if (Object.keys(patch).length === 0) {
      toast.message("Nothing changed");
      return;
    }
    setConfirmOpen(true);
  };

  const onConfirm = async () => {
    if (!cfg.data) return;
    const patch = diff(cfg.data, draft);
    try {
      const next = await update.mutateAsync(patch);
      setDraft(next);
      setConfirmOpen(false);
      toast.success("Config saved · live");
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "save failed");
    }
  };

  const set = <K extends keyof ConfigBody>(k: K, v: ConfigBody[K]) =>
    setDraft((d) => (d ? { ...d, [k]: v } : d));
  const setN =
    <K extends keyof ConfigBody>(k: K) =>
    (v: number | null) =>
      set(k, (v ?? 0) as ConfigBody[K]);
  const setNullable =
    <K extends keyof ConfigBody>(k: K) =>
    (v: number | null) =>
      set(k, v as ConfigBody[K]);

  return (
    <form onSubmit={onReview} className="px-3 pb-12 pt-4 md:px-6 md:pt-6">
      <header className="mb-5 flex flex-col items-start gap-3 md:mb-7 md:flex-row md:items-end md:justify-between md:gap-4">
        <div>
          <div className="eyebrow mb-1.5">Operations · Config</div>
          <h1 className="text-[22px] font-semibold leading-tight tracking-tight md:text-[28px]">
            Engine parameters
          </h1>
          <p className="mt-1.5 max-w-md font-mono text-[11.5px] text-muted-foreground">
            changes apply live · no restart required, unless noted
          </p>
        </div>
        <div className="flex w-full items-center gap-2 md:w-auto">
          <Button
            type="button"
            size="sm"
            variant="ghost"
            disabled={!dirty || update.isPending}
            onClick={() => setDraft(cfg.data ?? null)}
            title="Discard unsaved changes"
            className="h-8 flex-1 gap-1.5 px-3 text-[12px] md:flex-none"
          >
            <X className="size-3.5" strokeWidth={2} />
            Clear
          </Button>
          <Button
            type="submit"
            size="sm"
            disabled={update.isPending || !dirty}
            className="h-8 flex-1 gap-1.5 px-3 text-[12px] md:flex-none"
          >
            <Save className="size-3.5" strokeWidth={2} />
            {update.isPending ? "Saving…" : dirty ? "Save" : "Saved"}
          </Button>
        </div>
      </header>

      <div className="gap-4 md:columns-2 xl:columns-3 [&>*]:mb-4 [&>*]:break-inside-avoid">
        <Panel
          icon={Gauge}
          title="Bandwidth"
          description="Per-torrent simulated up/down caps. Decimal KB/s."
        >
          <RangeRow
            label="Upload range"
            hint="min · max · KB/s"
            minValue={draft.min_upload_speed}
            maxValue={draft.max_upload_speed}
            onMin={setN("min_upload_speed")}
            onMax={setN("max_upload_speed")}
          />
          <RangeRow
            label="Download range"
            hint="min · max · KB/s"
            minValue={draft.min_download_speed}
            maxValue={draft.max_download_speed}
            onMin={setN("min_download_speed")}
            onMax={setN("max_download_speed")}
          />
          <Row label="Bandwidth tick" hint="ms">
            <NumInput
              min={1}
              value={draft.bandwidth_tick_ms}
              onChange={setN("bandwidth_tick_ms")}
            />
          </Row>
          <AggregateCeiling
            maxUploadKBps={draft.max_upload_speed}
            maxActive={draft.max_active_torrents}
          />
        </Panel>

        <Panel
          icon={RotateCcw}
          title="Lifecycle"
          description="Active-slot cap, ratio target, eviction policy."
        >
          <Row label="Max active torrents">
            <NumInput
              min={1}
              value={draft.max_active_torrents}
              onChange={setN("max_active_torrents")}
            />
          </Row>
          <Row label="Upload ratio target" hint="-1 disables · 1.0 = full">
            <NumInput
              step="0.1"
              value={draft.upload_ratio_target}
              onChange={setN("upload_ratio_target")}
            />
          </Row>
          <ToggleRow
            label="Pause on zero leechers"
            hint="auto-pause when swarm reports no leechers"
            checked={draft.pause_torrent_with_zero_leechers}
            onChange={(v) => set("pause_torrent_with_zero_leechers", v)}
          />
          <Row label="Zero-leechers grace" hint="seconds before pause">
            <NumInput
              min={0}
              value={draft.pause_torrent_with_zero_leechers_grace}
              onChange={setN("pause_torrent_with_zero_leechers_grace")}
            />
          </Row>
          <Row label="Min swarm seeders" hint="0 = off · pause if scrape < N">
            <NumInput
              min={0}
              value={draft.min_swarm_seeders_to_seed}
              onChange={setN("min_swarm_seeders_to_seed")}
            />
          </Row>
        </Panel>

        <Panel
          icon={Globe2}
          title="Network"
          description="Announce port and concurrency limits."
        >
          <Row
            label="Announce port"
            hint="empty = use bound peer-port (default)"
          >
            <NumInput
              min={1}
              max={65535}
              value={draft.announce_port}
              onChange={setNullable("announce_port")}
              allowEmpty
              placeholder="auto"
            />
          </Row>
          <Row label="Max concurrent announces" hint="0 = unlimited">
            <NumInput
              min={0}
              value={draft.max_concurrent_announces}
              onChange={setN("max_concurrent_announces")}
            />
          </Row>
          <Row
            label="Announce jitter"
            hint="0 = off · adds 0–N s drift on reschedule"
          >
            <NumInput
              min={0}
              value={draft.max_announce_jitter}
              onChange={setN("max_announce_jitter")}
            />
          </Row>
          <ConnectivityRow port={draft.announce_port} dirty={!!dirty} />
        </Panel>

        <Panel
          icon={Cloud}
          title="Tracker HTTP client"
          description="Reqwest knobs · read at startup, restart to apply."
          muted
        >
          {(
            [
              [
                "http_tracker_connect_timeout_secs",
                "Connect timeout",
                "seconds",
              ],
              [
                "http_tracker_request_timeout_secs",
                "Request timeout",
                "seconds",
              ],
              ["http_tracker_max_idle_per_host", "Max idle / host", "count"],
              ["http_tracker_max_redirects", "Max redirects", "count"],
              ["http_tracker_tcp_keepalive_secs", "TCP keepalive", "seconds"],
              [
                "http_tracker_pool_idle_timeout_secs",
                "Pool idle timeout",
                "seconds",
              ],
            ] as const
          ).map(([k, label, hint]) => (
            <Row key={k} label={label} hint={hint}>
              <NumInput
                min={0}
                placeholder="default"
                value={draft[k] ?? ""}
                onChange={setNullable(k)}
                allowEmpty
              />
            </Row>
          ))}
        </Panel>

        <section className="@container flex flex-col overflow-hidden rounded-md border border-destructive/30 bg-destructive/[0.03]">
          <header className="flex items-center gap-2 border-b border-destructive/20 px-4 py-2.5 md:px-5">
            <span
              className="inline-flex size-5 items-center justify-center rounded-sm bg-destructive/15 text-destructive"
              aria-hidden="true"
            >
              <AlertTriangle className="size-3" strokeWidth={1.75} />
            </span>
            <h2 className="text-[13px] font-semibold leading-none tracking-tight text-destructive">
              Danger zone
            </h2>
          </header>
          <div className="flex flex-col items-start gap-3 bg-background px-4 py-3 md:px-5 @md:flex-row @md:items-center @md:justify-between">
            <div className="min-w-0">
              <div className="text-[12.5px] font-medium leading-tight">
                Reset to engine defaults
              </div>
              <div className="mt-0.5 font-mono text-[10.5px] uppercase tracking-wider text-muted-foreground/70">
                loads compile-time defaults · review and save
              </div>
            </div>
            <Button
              type="button"
              size="sm"
              variant="outline"
              disabled={resetting || update.isPending}
              onClick={onReset}
              className="h-8 shrink-0 gap-1.5 border-destructive/40 px-3 text-[12px] text-destructive hover:bg-destructive/10 hover:text-destructive"
            >
              <RotateCcw className="size-3.5" strokeWidth={2} />
              {resetting ? "Loading…" : "Reset to defaults"}
            </Button>
          </div>
        </section>
      </div>

      <ConfirmChangesDialog
        open={confirmOpen}
        onOpenChange={setConfirmOpen}
        prev={cfg.data ?? null}
        next={draft}
        pending={update.isPending}
        onConfirm={onConfirm}
      />
    </form>
  );
}

/* ─────────────────── Panel + row primitives ─────────────────── */

function Panel({
  icon: Icon,
  title,
  description,
  children,
  muted,
  className,
}: {
  icon: LucideIcon;
  title: string;
  description?: string;
  children: React.ReactNode;
  muted?: boolean;
  className?: string;
}) {
  return (
    <section
      className={cn(
        "@container flex flex-col overflow-hidden rounded-md border bg-card",
        muted && "bg-card/40",
        className,
      )}
    >
      <header className="flex flex-col gap-1 border-b px-4 py-2.5 @md:flex-row @md:items-center @md:justify-between @md:gap-3 md:px-5">
        <div className="flex shrink-0 items-center gap-2">
          <span
            className="inline-flex size-5 items-center justify-center rounded-sm bg-muted text-muted-foreground"
            aria-hidden="true"
          >
            <Icon className="size-3" strokeWidth={1.75} />
          </span>
          <h2 className="text-[13px] font-semibold leading-none tracking-tight">
            {title}
          </h2>
        </div>
        {description && (
          <p className="font-mono text-[10.5px] leading-tight text-muted-foreground/70 @md:min-w-0 @md:truncate @md:leading-none @md:text-right">
            {description}
          </p>
        )}
      </header>
      <div className="flex-1 divide-y divide-border">{children}</div>
    </section>
  );
}

function Row({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex min-h-[3.25rem] items-center justify-between gap-4 bg-background px-4 py-2.5 md:px-5">
      <div className="min-w-0 flex-1">
        <div className="text-[12.5px] font-medium leading-tight">{label}</div>
        {hint && (
          <div className="mt-0.5 font-mono text-[10.5px] uppercase tracking-wider leading-tight text-muted-foreground/70">
            {hint}
          </div>
        )}
      </div>
      <div className="flex shrink-0 items-center">{children}</div>
    </div>
  );
}

function ConnectivityRow({
  port,
  dirty,
}: {
  port: number | null;
  dirty: boolean;
}) {
  const probe = useCheckConnectivity();
  const result = probe.data ?? null;

  const onClick = () => {
    probe.mutate(port ?? undefined);
  };

  return (
    <div className="flex flex-col gap-2 bg-background px-4 py-3 md:px-5">
      <div className="flex items-center justify-between gap-4">
        <div className="min-w-0 flex-1">
          <div className="text-[12.5px] font-medium leading-tight">
            Reachability
          </div>
          <div className="mt-0.5 font-mono text-[10.5px] uppercase tracking-wider leading-tight text-muted-foreground/70">
            {dirty ? "save first to test current port" : "probes ifconfig.co"}
          </div>
        </div>
        <Button
          type="button"
          size="sm"
          variant="outline"
          disabled={dirty || probe.isPending}
          onClick={onClick}
          className="h-8 shrink-0 gap-1.5 px-3 text-[12px]"
        >
          {probe.isPending ? (
            <Loader2 className="size-3.5 animate-spin" strokeWidth={2} />
          ) : (
            <Wifi className="size-3.5" strokeWidth={2} />
          )}
          {probe.isPending ? "Probing…" : "Check"}
        </Button>
      </div>
      {result && (
        <div className="flex flex-col gap-1.5 rounded-md border border-border/70 bg-muted/30 px-3 py-2">
          <div className="flex items-center justify-between font-mono text-[10.5px] uppercase tracking-wider text-muted-foreground/70">
            <span>port {result.port}</span>
            <span>{new Date(result.checked_at_ms).toLocaleTimeString()}</span>
          </div>
          <FamilyLine label="IPv4" family={result.ipv4} />
          <FamilyLine label="IPv6" family={result.ipv6} />
        </div>
      )}
      {probe.isError && (
        <div className="rounded-md border border-destructive/30 bg-destructive/[0.06] px-3 py-2 font-mono text-[11px] text-destructive">
          probe failed:{" "}
          {probe.error instanceof Error ? probe.error.message : "unknown error"}
        </div>
      )}
    </div>
  );
}

function FamilyLine({
  label,
  family,
}: {
  label: string;
  family: ConnectivityFamily;
}) {
  const reachable = family.reachable;
  const detail = family.public_ip ?? family.error ?? "—";
  return (
    <div className="flex items-center justify-between gap-3 text-[12px]">
      <div className="flex items-center gap-1.5">
        {reachable ? (
          <CheckCircle2
            className="size-3.5 text-success"
            strokeWidth={2}
            aria-hidden
          />
        ) : (
          <XCircle
            className="size-3.5 text-destructive/80"
            strokeWidth={2}
            aria-hidden
          />
        )}
        <span className="font-mono text-[11px] font-medium uppercase tracking-wider">
          {label}
        </span>
        <span
          className={cn(
            "text-[11px] font-medium",
            reachable ? "text-success" : "text-muted-foreground",
          )}
        >
          {reachable ? "reachable" : "unreachable"}
        </span>
      </div>
      <span className="truncate font-mono text-[11px] text-muted-foreground/80">
        {detail}
      </span>
    </div>
  );
}

function RangeRow({
  label,
  hint,
  minValue,
  maxValue,
  onMin,
  onMax,
}: {
  label: string;
  hint?: string;
  minValue: number;
  maxValue: number;
  onMin: (v: number | null) => void;
  onMax: (v: number | null) => void;
}) {
  return (
    <div className="flex min-h-[3.25rem] items-center justify-between gap-4 bg-background px-4 py-2.5 md:px-5">
      <div className="min-w-0 flex-1">
        <div className="text-[12.5px] font-medium leading-tight">{label}</div>
        <div className="mt-0.5 font-mono text-[10.5px] uppercase tracking-wider leading-tight text-muted-foreground/70">
          {hint ?? "min · max"}
        </div>
      </div>
      <div className="flex shrink-0 items-center gap-1.5">
        <NumInput min={0} value={minValue} onChange={onMin} />
        <span
          className="font-mono text-[11px] text-muted-foreground/60"
          aria-hidden="true"
        >
          —
        </span>
        <NumInput min={0} value={maxValue} onChange={onMax} />
      </div>
    </div>
  );
}

const PLAUSIBILITY_TIERS: {
  threshold: number;
  short: string;
  full: string;
  pill: string;
}[] = [
  { threshold: 0, short: "10", full: "avg residential", pill: "residential" },
  { threshold: 50, short: "50", full: "high residential", pill: "fiber" },
  { threshold: 100, short: "100", full: "seedbox", pill: "seedbox" },
  {
    threshold: 1000,
    short: "1G",
    full: "high-end seedbox",
    pill: "high-end",
  },
  {
    threshold: 10000,
    short: "10G",
    full: "10 Gbps+ seedbox",
    pill: "10G+",
  },
];

const SCALE_MIN = 10;
const SCALE_MAX = 10000;
const logPct = (mbps: number) => {
  if (mbps <= SCALE_MIN) return 0;
  if (mbps >= SCALE_MAX) return 100;
  return (
    (Math.log10(mbps / SCALE_MIN) / Math.log10(SCALE_MAX / SCALE_MIN)) * 100
  );
};

const fmtMbps = (mbps: number) => {
  if (mbps >= 1000) return (mbps / 1000).toFixed(mbps >= 10000 ? 0 : 1);
  if (mbps >= 100) return mbps.toFixed(0);
  if (mbps >= 10) return mbps.toFixed(1);
  if (mbps >= 1) return mbps.toFixed(2);
  return mbps.toFixed(2);
};

const mbpsUnit = (mbps: number) => (mbps >= 1000 ? "Gbps" : "Mbps");

function AggregateCeiling({
  maxUploadKBps,
  maxActive,
}: {
  maxUploadKBps: number;
  maxActive: number;
}) {
  const aggKBps = Math.max(0, maxUploadKBps) * Math.max(0, maxActive);
  const mbps = (aggKBps * 8) / 1000;
  const mBPerS = aggKBps / 1000;

  let tone: "ok" | "warn" | "amber" | "red";
  if (mbps > 10000) tone = "red";
  else if (mbps > 1000) tone = "amber";
  else if (mbps > 100) tone = "warn";
  else tone = "ok";

  const activeIdx = PLAUSIBILITY_TIERS.reduce(
    (acc, t, i) => (mbps >= t.threshold ? i : acc),
    0,
  );
  const active = PLAUSIBILITY_TIERS[activeIdx];

  const wrapCls =
    tone === "red"
      ? "border-destructive/30 bg-destructive/[0.04]"
      : tone === "amber"
        ? "border-amber-500/30 bg-amber-500/[0.05]"
        : tone === "warn"
          ? "border-yellow-500/30 bg-yellow-500/[0.05]"
          : "border-border/60 bg-muted/40";

  const fillStyle =
    tone === "red"
      ? "bg-gradient-to-r from-destructive/40 via-destructive/70 to-destructive"
      : tone === "amber"
        ? "bg-gradient-to-r from-amber-500/40 via-amber-500/70 to-amber-500"
        : tone === "warn"
          ? "bg-gradient-to-r from-yellow-500/40 via-yellow-500/65 to-yellow-500/85"
          : "bg-gradient-to-r from-emerald-500/40 via-emerald-500/65 to-emerald-500/85";

  const dotCls =
    tone === "red"
      ? "bg-destructive"
      : tone === "amber"
        ? "bg-amber-500"
        : tone === "warn"
          ? "bg-yellow-500"
          : "bg-emerald-500";

  const textTone =
    tone === "red"
      ? "text-destructive"
      : tone === "amber"
        ? "text-amber-700 dark:text-amber-400"
        : tone === "warn"
          ? "text-yellow-700 dark:text-yellow-400"
          : "text-emerald-700 dark:text-emerald-400";

  const pillCls =
    tone === "red"
      ? "border-destructive/40 bg-destructive/10 text-destructive"
      : tone === "amber"
        ? "border-amber-500/40 bg-amber-500/10 text-amber-700 dark:text-amber-400"
        : tone === "warn"
          ? "border-yellow-500/40 bg-yellow-500/10 text-yellow-700 dark:text-yellow-400"
          : "border-emerald-500/40 bg-emerald-500/10 text-emerald-700 dark:text-emerald-400";

  const valuePct = logPct(mbps);

  return (
    <div className={cn("border-t px-4 py-3.5 md:px-5 md:py-4", wrapCls)}>
      <div className="flex items-center justify-between gap-2">
        <span className="font-mono text-[9.5px] uppercase leading-none tracking-[0.18em] text-muted-foreground/75">
          Plausibility ceiling
        </span>
        <span
          className={cn(
            "inline-flex items-center gap-1 rounded-full border px-2 py-[2px] font-mono text-[9.5px] uppercase leading-none tracking-wider",
            pillCls,
          )}
        >
          <span
            aria-hidden="true"
            className={cn("size-1.5 rounded-full", dotCls)}
          />
          {active.pill}
        </span>
      </div>

      <div className="mt-3 flex items-baseline gap-1.5 leading-none">
        <span
          className={cn(
            "text-[30px] font-semibold tabular-nums tracking-tight md:text-[34px]",
            textTone,
          )}
        >
          {fmtMbps(mbps)}
        </span>
        <span className="font-mono text-[11px] uppercase tracking-wider text-muted-foreground">
          {mbpsUnit(mbps)}
        </span>
      </div>
      <div className="mt-1 text-[11.5px] font-medium leading-tight text-foreground/80">
        {active.full}
      </div>

      <div className="mt-2 font-mono text-[10.5px] uppercase tracking-wider leading-tight text-muted-foreground/70">
        {maxActive} × {maxUploadKBps} KB/s · {mBPerS.toFixed(1)} MB/s
      </div>

      <div className="mt-3.5 select-none">
        <div className="relative h-2 w-full rounded-full bg-foreground/[0.08] shadow-[inset_0_1px_0_0_rgba(0,0,0,0.04)]">
          {PLAUSIBILITY_TIERS.slice(1, -1).map((t) => {
            const left = logPct(t.threshold);
            return (
              <span
                key={t.threshold}
                aria-hidden="true"
                className="absolute top-1/2 h-3 w-px -translate-x-1/2 -translate-y-1/2 bg-foreground/25"
                style={{ left: `${left}%` }}
              />
            );
          })}
          <span
            className={cn(
              "absolute inset-y-0 left-0 rounded-full transition-[width] duration-300 ease-out",
              fillStyle,
            )}
            style={{ width: `${valuePct}%` }}
          />
          <span
            aria-hidden="true"
            className={cn(
              "absolute top-1/2 size-3 -translate-x-1/2 -translate-y-1/2 rounded-full ring-2 ring-background shadow-sm transition-[left] duration-300 ease-out",
              dotCls,
            )}
            style={{ left: `${valuePct}%` }}
          />
        </div>

        <div className="relative mt-2 h-3 w-full">
          {PLAUSIBILITY_TIERS.map((t, i) => {
            const left = logPct(t.threshold);
            const isActive = i === activeIdx;
            const align =
              i === 0
                ? "translate-x-0"
                : i === PLAUSIBILITY_TIERS.length - 1
                  ? "-translate-x-full"
                  : "-translate-x-1/2";
            return (
              <span
                key={t.threshold}
                className={cn(
                  "absolute top-0 font-mono text-[9.5px] uppercase tabular-nums leading-none tracking-wider transition-colors",
                  align,
                  isActive
                    ? cn("font-semibold", textTone)
                    : "text-muted-foreground/55",
                )}
                style={{ left: `${left}%` }}
              >
                {t.short}
              </span>
            );
          })}
        </div>
      </div>

      {tone !== "ok" && (
        <div
          className={cn(
            "mt-3 flex items-start gap-2 rounded-md border px-2.5 py-2",
            tone === "red"
              ? "border-destructive/20 bg-destructive/[0.05]"
              : tone === "amber"
                ? "border-amber-500/25 bg-amber-500/[0.06]"
                : "border-yellow-500/25 bg-yellow-500/[0.06]",
          )}
        >
          <AlertTriangle
            className={cn("mt-[1px] size-3 shrink-0", textTone)}
            strokeWidth={1.75}
            aria-hidden="true"
          />
          <span className={cn("text-[11px] leading-snug", textTone)}>
            {tone === "warn"
              ? "Past residential — only plausible if you've declared a seedbox to the tracker."
              : tone === "amber"
                ? "Past avg seedbox — implies a high-end seedbox tier (1 Gbps+)."
                : "Past 10 Gbps — implausible for any commercial connection. Almost-certain ban trigger."}
          </span>
        </div>
      )}
    </div>
  );
}

function ToggleRow({
  label,
  hint,
  checked,
  onChange,
}: {
  label: string;
  hint?: string;
  checked: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      onClick={() => onChange(!checked)}
      className="flex min-h-[3.25rem] w-full items-center gap-3 bg-background px-4 py-2.5 text-left transition-colors hover:bg-accent/40 md:px-5"
    >
      <div className="min-w-0 flex-1">
        <div className="text-[12.5px] font-medium leading-tight">{label}</div>
        {hint && (
          <div className="mt-0.5 font-mono text-[10.5px] leading-tight text-muted-foreground/80">
            {hint}
          </div>
        )}
      </div>
      <span
        className={cn(
          "relative inline-flex h-5 w-9 shrink-0 items-center rounded-full border transition-colors",
          checked ? "border-success bg-success" : "border-border bg-muted",
        )}
        aria-hidden="true"
      >
        <span
          className={cn(
            "inline-block size-3.5 rounded-full bg-background shadow-sm transition-transform",
            checked ? "translate-x-[1.125rem]" : "translate-x-[0.125rem]",
          )}
        />
      </span>
    </button>
  );
}

type NumInputProps = {
  value: number | string | null;
  onChange: (v: number | null) => void;
  min?: number;
  max?: number;
  step?: string;
  placeholder?: string;
  allowEmpty?: boolean;
};

function NumInput({
  value,
  onChange,
  min,
  max,
  step,
  placeholder,
  allowEmpty,
}: NumInputProps) {
  const canonical = value === null || value === undefined ? "" : String(value);
  const [text, setText] = useState(canonical);
  // Sync from outside when parent value changes and our buffer is in a
  // committed (parsed) state — never clobber the user's mid-typing input.
  useEffect(() => {
    if (text === "" || text === "-" || text.endsWith(".")) return;
    if (Number(text) !== Number(canonical)) setText(canonical);
  }, [canonical, text]);
  const commit = (raw: string) => {
    if (raw === "" || raw === "-") {
      onChange(allowEmpty ? null : 0);
      return;
    }
    const n = Number(raw);
    if (Number.isFinite(n)) onChange(n);
  };
  return (
    <Input
      type="text"
      inputMode="decimal"
      value={text}
      placeholder={placeholder}
      onChange={(e) => {
        const v = e.target.value;
        if (!/^-?\d*\.?\d*$/.test(v)) return;
        setText(v);
        commit(v);
      }}
      onBlur={() => {
        if (text === "" || text === "-") {
          setText(canonical);
        }
      }}
      data-min={min}
      data-max={max}
      data-step={step}
      className="h-8 w-[9ch] px-2 text-right font-mono text-[12px] tabular-nums"
    />
  );
}

/* ─────────────────── Confirm dialog (unchanged) ─────────────────── */

function ConfirmChangesDialog({
  open,
  onOpenChange,
  prev,
  next,
  pending,
  onConfirm,
}: {
  open: boolean;
  onOpenChange: (v: boolean) => void;
  prev: ConfigBody | null;
  next: ConfigBody;
  pending: boolean;
  onConfirm: () => void;
}) {
  const patch = prev ? diff(prev, next) : {};
  const keys = Object.keys(patch) as (keyof ConfigBody)[];
  const items: DiffListItem[] = keys.map((k) => ({
    key: String(k),
    from: fmtVal(prev?.[k]),
    to: fmtVal(next[k]),
  }));
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <span className="eyebrow-strong">
            Review · {keys.length} change{keys.length === 1 ? "" : "s"}
          </span>
          <DialogTitle className="text-base font-semibold">
            Apply configuration
          </DialogTitle>
          <DialogDescription className="text-[12px]">
            Changes apply live to the running engine.
          </DialogDescription>
        </DialogHeader>

        <DiffList items={items} />

        <DialogFooter>
          <Button
            type="button"
            variant="ghost"
            onClick={() => onOpenChange(false)}
            disabled={pending}
          >
            Cancel
          </Button>
          <Button type="button" onClick={onConfirm} disabled={pending}>
            {pending ? "Applying…" : "Apply changes"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function fmtVal(v: unknown): string {
  if (v === null || v === undefined) return "default";
  if (typeof v === "boolean") return v ? "on" : "off";
  return String(v);
}

function diff(prev: ConfigBody, next: ConfigBody): ConfigUpdate {
  const out: ConfigUpdate = {};
  for (const k of Object.keys(next) as (keyof ConfigBody)[]) {
    if (prev[k] !== next[k]) {
      (out as Record<string, unknown>)[k] = next[k];
    }
  }
  return out;
}
