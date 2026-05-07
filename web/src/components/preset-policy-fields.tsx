// Policy form fields, reused in the preset edit dialog and (eventually) inline editor.

import { ChevronDown } from "lucide-react";
import { useState } from "react";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useProfiles } from "@/lib/queries";
import type { PresetPolicy } from "@/lib/types";
import { cn } from "@/lib/utils";

export interface PolicyFieldsProps {
  value: PresetPolicy;
  onChange: (next: PresetPolicy) => void;
  /** Hide the AggregateCeiling banner when nested inside a small surface. */
  hideCeiling?: boolean;
}

export function PresetPolicyFields({
  value,
  onChange,
  hideCeiling,
}: PolicyFieldsProps) {
  const [advanced, setAdvanced] = useState(false);
  const set = <K extends keyof PresetPolicy>(k: K, v: PresetPolicy[K]) =>
    onChange({ ...value, [k]: v });

  return (
    <div className="space-y-4">
      <Section title="Bandwidth" hint="per-torrent simulated caps · KB/s">
        <Range
          label="Upload range"
          minValue={value.min_upload_speed}
          maxValue={value.max_upload_speed}
          onMin={(v) => set("min_upload_speed", v)}
          onMax={(v) => set("max_upload_speed", v)}
        />
        <Range
          label="Download range"
          minValue={value.min_download_speed}
          maxValue={value.max_download_speed}
          onMin={(v) => set("min_download_speed", v)}
          onMax={(v) => set("max_download_speed", v)}
        />
      </Section>

      <Section title="Identity" hint="how torrents in this preset announce">
        <ProfileRow
          value={value.client_profile_id}
          onChange={(v) => set("client_profile_id", v)}
        />
      </Section>

      <Section title="Lifecycle" hint="active slots, ratio cap, eviction">
        <Row label="Max active torrents" hint="concurrent in this preset">
          <Num
            min={1}
            value={value.max_active_torrents}
            onChange={(v) => set("max_active_torrents", v)}
          />
        </Row>
        <Row label="Upload ratio target" hint="-1 disables · 1.0 = full">
          <Num
            step="0.1"
            value={value.upload_ratio_target}
            onChange={(v) => set("upload_ratio_target", v)}
          />
        </Row>
        <Row label="Min swarm seeders" hint="0 = off · pause when scrape < N">
          <Num
            min={0}
            value={value.min_swarm_seeders_to_seed}
            onChange={(v) => set("min_swarm_seeders_to_seed", v)}
          />
        </Row>
      </Section>

      {!hideCeiling && <AggregateCeiling policy={value} />}

      <button
        type="button"
        onClick={() => setAdvanced((v) => !v)}
        className={cn(
          "group flex w-full cursor-pointer items-center justify-between rounded-md border border-dashed border-border/70 bg-card/30 px-3 py-2 text-left transition-colors hover:bg-foreground/[0.03]",
        )}
      >
        <span className="font-mono text-[10.5px] uppercase tracking-wider text-muted-foreground">
          {advanced ? "Hide" : "Show"} advanced
        </span>
        <ChevronDown
          className={cn(
            "size-3.5 text-muted-foreground transition-transform",
            advanced && "rotate-180",
          )}
          strokeWidth={2}
        />
      </button>

      {advanced && (
        <Section title="Anti-detection" hint="zero-leecher pause + jitter">
          <Toggle
            label="Pause on zero leechers"
            hint="auto-pause when swarm has no leechers"
            checked={value.pause_torrent_with_zero_leechers}
            onChange={(v) => set("pause_torrent_with_zero_leechers", v)}
          />
          <Row label="Zero-leechers grace" hint="seconds before pause fires">
            <Num
              min={0}
              value={value.pause_torrent_with_zero_leechers_grace}
              onChange={(v) => set("pause_torrent_with_zero_leechers_grace", v)}
            />
          </Row>
          <Row label="Announce jitter" hint="0–N s drift on reschedule">
            <Num
              min={0}
              value={value.max_announce_jitter}
              onChange={(v) => set("max_announce_jitter", v)}
            />
          </Row>
        </Section>
      )}
    </div>
  );
}

function Section({
  title,
  hint,
  children,
}: {
  title: string;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <section className="overflow-hidden rounded-md border bg-card">
      <header className="flex items-baseline justify-between border-b border-border/60 px-3 py-2 md:px-4">
        <span className="font-mono text-[10.5px] font-medium uppercase tracking-[0.18em] text-foreground/85">
          {title}
        </span>
        {hint && (
          <span className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground/60">
            {hint}
          </span>
        )}
      </header>
      <div className="divide-y divide-border/40 bg-background">{children}</div>
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
    <div className="flex min-h-[3rem] items-center justify-between gap-3 px-3 py-2 md:px-4">
      <div className="min-w-0 flex-1">
        <div className="text-[12.5px] font-medium leading-tight">{label}</div>
        {hint && (
          <div className="mt-0.5 font-mono text-[10px] uppercase tracking-wider text-muted-foreground/65">
            {hint}
          </div>
        )}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

function Range({
  label,
  minValue,
  maxValue,
  onMin,
  onMax,
}: {
  label: string;
  minValue: number;
  maxValue: number;
  onMin: (v: number) => void;
  onMax: (v: number) => void;
}) {
  return (
    <div className="flex min-h-[3rem] items-center justify-between gap-3 px-3 py-2 md:px-4">
      <div className="min-w-0 flex-1">
        <div className="text-[12.5px] font-medium leading-tight">{label}</div>
        <div className="mt-0.5 font-mono text-[10px] uppercase tracking-wider text-muted-foreground/65">
          min · max · KB/s
        </div>
      </div>
      <div className="flex shrink-0 items-center gap-1.5">
        <Num min={0} value={minValue} onChange={onMin} />
        <span
          aria-hidden="true"
          className="font-mono text-[11px] text-muted-foreground/60"
        >
          —
        </span>
        <Num min={0} value={maxValue} onChange={onMax} />
      </div>
    </div>
  );
}

function Toggle({
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
  const id = `toggle-${label.replace(/\s+/g, "-").toLowerCase()}`;
  return (
    <div className="flex items-center justify-between gap-3 px-3 py-2 md:px-4">
      <Label
        htmlFor={id}
        className="min-w-0 flex-1 cursor-pointer text-[12.5px] font-medium leading-tight"
      >
        {label}
        {hint && (
          <span className="mt-0.5 block font-mono text-[10px] font-normal uppercase tracking-wider text-muted-foreground/65">
            {hint}
          </span>
        )}
      </Label>
      <Checkbox
        id={id}
        checked={checked}
        onCheckedChange={(v) => onChange(!!v)}
      />
    </div>
  );
}

function Num({
  value,
  onChange,
  min,
  max,
  step,
}: {
  value: number;
  onChange: (v: number) => void;
  min?: number;
  max?: number;
  step?: string;
}) {
  return (
    <Input
      type="number"
      min={min}
      max={max}
      step={step}
      className="h-7 w-20 px-2 text-right font-mono text-[12.5px] tabular-nums"
      value={Number.isFinite(value) ? value : ""}
      onChange={(e) => {
        const v = e.currentTarget.value;
        const n = v === "" ? 0 : Number(v);
        onChange(Number.isFinite(n) ? n : 0);
      }}
    />
  );
}

/* ---- Aggregate ceiling (compact, in-form variant) ---- */

const SCALE_MIN = 10;
const SCALE_MAX = 10000;
const logPct = (mbps: number) => {
  if (mbps <= SCALE_MIN) return 0;
  if (mbps >= SCALE_MAX) return 100;
  return (
    (Math.log10(mbps / SCALE_MIN) / Math.log10(SCALE_MAX / SCALE_MIN)) * 100
  );
};

const TIERS = [
  { mbps: 50, short: "50" },
  { mbps: 100, short: "100" },
  { mbps: 1000, short: "1G" },
  { mbps: 10000, short: "10G" },
];

function fmtMbps(mbps: number): string {
  if (mbps >= 10000) return (mbps / 1000).toFixed(0);
  if (mbps >= 1000) return (mbps / 1000).toFixed(1);
  if (mbps >= 100) return mbps.toFixed(0);
  if (mbps >= 10) return mbps.toFixed(1);
  return mbps.toFixed(2);
}

function AggregateCeiling({ policy }: { policy: PresetPolicy }) {
  const aggKBps =
    Math.max(0, policy.max_upload_speed) *
    Math.max(0, policy.max_active_torrents);
  const mbps = (aggKBps * 8) / 1000;
  const mBPerS = aggKBps / 1000;

  let tone: "ok" | "warn" | "amber" | "red";
  if (mbps > 10000) tone = "red";
  else if (mbps > 1000) tone = "amber";
  else if (mbps > 100) tone = "warn";
  else tone = "ok";

  const fillCls =
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

  const valuePct = logPct(mbps);

  return (
    <section className="overflow-hidden rounded-md border bg-card">
      <header className="flex items-baseline justify-between border-b border-border/60 px-3 py-2 md:px-4">
        <span className="font-mono text-[10.5px] font-medium uppercase tracking-[0.18em] text-foreground/85">
          Plausibility
        </span>
        <span className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground/65">
          aggregate upload
        </span>
      </header>
      <div className="bg-background px-3 py-3 md:px-4">
        <div className="flex items-baseline gap-1.5 leading-none">
          <span className="text-[24px] font-semibold tabular-nums tracking-tight md:text-[28px]">
            {fmtMbps(mbps)}
          </span>
          <span className="font-mono text-[11px] uppercase tracking-wider text-muted-foreground">
            {mbps >= 1000 ? "Gbps" : "Mbps"}
          </span>
        </div>
        <div className="mt-1 font-mono text-[10.5px] uppercase tracking-wider text-muted-foreground/70">
          {policy.max_active_torrents} × {policy.max_upload_speed} KB/s ·{" "}
          {mBPerS.toFixed(1)} MB/s
        </div>

        <div className="relative mt-3 h-1.5 w-full rounded-full bg-foreground/[0.08]">
          {TIERS.slice(0, -1).map((t) => (
            <span
              key={t.mbps}
              aria-hidden="true"
              className="absolute top-1/2 h-2.5 w-px -translate-x-1/2 -translate-y-1/2 bg-foreground/25"
              style={{ left: `${logPct(t.mbps)}%` }}
            />
          ))}
          <span
            className={cn(
              "absolute inset-y-0 left-0 rounded-full transition-[width] duration-300",
              fillCls,
            )}
            style={{ width: `${valuePct}%` }}
          />
          <span
            aria-hidden="true"
            className={cn(
              "absolute top-1/2 size-2.5 -translate-x-1/2 -translate-y-1/2 rounded-full ring-2 ring-background",
              dotCls,
            )}
            style={{ left: `${valuePct}%` }}
          />
        </div>
        <div className="relative mt-2 h-3 w-full">
          {TIERS.map((t, i) => (
            <span
              key={t.mbps}
              className={cn(
                "absolute top-0 font-mono text-[9.5px] uppercase tabular-nums leading-none tracking-wider text-muted-foreground/55",
                i === TIERS.length - 1 && "-translate-x-full",
                i !== 0 && i !== TIERS.length - 1 && "-translate-x-1/2",
              )}
              style={{ left: `${logPct(t.mbps)}%` }}
            >
              {t.short}
            </span>
          ))}
        </div>
      </div>
    </section>
  );
}

/* ---- Client profile picker (per-preset identity, two-step) ---- */

const SELECT_CLS = cn(
  "h-7 rounded-md border border-input bg-transparent px-2 text-[12px] outline-none transition-colors",
  "focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50",
  "disabled:cursor-not-allowed disabled:opacity-50",
);

function ProfileRow({
  value,
  onChange,
}: {
  value: string | null;
  onChange: (v: string | null) => void;
}) {
  const { data: profiles } = useProfiles();
  const list = profiles ?? [];

  // Group all variants by client family. Variants are sorted natural-descending
  // (newest first) within each client.
  const byClient = new Map<string, typeof list>();
  for (const p of list) {
    const arr = byClient.get(p.client) ?? [];
    arr.push(p);
    byClient.set(p.client, arr);
  }
  for (const [_, arr] of byClient) {
    arr.sort((a, b) =>
      (b.version || b.id).localeCompare(a.version || a.id, undefined, {
        numeric: true,
        sensitivity: "base",
      }),
    );
  }
  // Sort client families alphabetically for stable order in the first select.
  const clientNames = Array.from(byClient.keys()).sort((a, b) =>
    a.localeCompare(b),
  );

  // Decompose the persisted variant id (`client@version`) into its parts.
  const selected = value ? list.find((p) => p.id === value) : null;
  const selectedClient = selected?.client ?? "";
  const variantsForSelected = selectedClient
    ? (byClient.get(selectedClient) ?? [])
    : [];

  const defaultActive = list.find((p) => p.active);
  const defaultLabel = defaultActive
    ? `Default (${defaultActive.id})`
    : "Default";

  const onClientChange = (client: string) => {
    if (!client) {
      onChange(null);
      return;
    }
    // When switching client family, default to the first variant of that client.
    const first = byClient.get(client)?.[0];
    if (first) onChange(first.id);
  };

  const onVariantChange = (variantId: string) => {
    onChange(variantId);
  };

  return (
    <div className="flex min-h-[3rem] flex-col items-stretch gap-2 px-3 py-2 md:flex-row md:items-center md:justify-between md:px-4">
      <div className="min-w-0 flex-1">
        <div className="text-[12.5px] font-medium leading-tight">
          Client profile
        </div>
        <div className="mt-0.5 font-mono text-[10px] uppercase tracking-wider text-muted-foreground/65">
          peer_id + key + headers used on announce
        </div>
      </div>
      <div className="flex shrink-0 items-center gap-1.5">
        <select
          value={value === null ? "" : selectedClient}
          onChange={(e) => onClientChange(e.currentTarget.value)}
          className={cn(SELECT_CLS, "max-w-[10rem]")}
          aria-label="Client family"
        >
          <option value="">{defaultLabel}</option>
          {clientNames.map((client) => (
            <option key={client} value={client}>
              {client}
            </option>
          ))}
        </select>
        <select
          value={value ?? ""}
          onChange={(e) => onVariantChange(e.currentTarget.value)}
          disabled={!selectedClient}
          className={cn(SELECT_CLS, "max-w-[8rem] tabular-nums")}
          aria-label="Variant"
        >
          {!selectedClient && <option value="">—</option>}
          {variantsForSelected.map((p) => (
            <option key={p.id} value={p.id}>
              {p.version || p.id}
            </option>
          ))}
        </select>
      </div>
    </div>
  );
}
