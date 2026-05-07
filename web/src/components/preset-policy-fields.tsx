// Reusable policy-fields group. Bound via `withFieldGroup` to a sub-tree of any
// form whose shape contains `policy: PresetPolicy`.
//
// Usage:
//   <PresetPolicyFields form={form} fields="policy" />

import { ChevronDown } from "lucide-react";
import { useState } from "react";

import { withFieldGroup } from "@/lib/form-hook";
import { DEFAULT_POLICY, type PresetPolicy } from "@/lib/schemas";
import { cn } from "@/lib/utils";

export const PresetPolicyFields = withFieldGroup({
  defaultValues: DEFAULT_POLICY,
  props: { hideCeiling: false as boolean },
  render: function Render({ group, hideCeiling }) {
    const [advanced, setAdvanced] = useState(false);
    return (
      <div className="space-y-4">
        <Section title="Bandwidth" hint="per-torrent simulated caps · KB/s">
          <RangeRow
            label="Upload range"
            renderMin={() => (
              <group.AppField name="min_upload_speed">
                {(field) => <field.NumberInput min={0} />}
              </group.AppField>
            )}
            renderMax={() => (
              <group.AppField name="max_upload_speed">
                {(field) => <field.NumberInput min={0} />}
              </group.AppField>
            )}
          />
          <RangeRow
            label="Download range"
            renderMin={() => (
              <group.AppField name="min_download_speed">
                {(field) => <field.NumberInput min={0} />}
              </group.AppField>
            )}
            renderMax={() => (
              <group.AppField name="max_download_speed">
                {(field) => <field.NumberInput min={0} />}
              </group.AppField>
            )}
          />
        </Section>

        <Section title="Identity" hint="how torrents in this preset announce">
          <group.AppField name="client_profile_id">
            {(field) => <field.ClientProfileField />}
          </group.AppField>
        </Section>

        <Section title="Lifecycle" hint="active slots, ratio cap, eviction">
          <group.AppField name="max_active_torrents">
            {(field) => (
              <field.NumberRow
                label="Max active torrents"
                hint="concurrent in this preset"
                min={1}
              />
            )}
          </group.AppField>
          <group.AppField name="upload_ratio_target">
            {(field) => (
              <field.NumberRow
                label="Upload ratio target"
                hint="-1 disables · 1.0 = full"
                step="0.1"
              />
            )}
          </group.AppField>
          <group.AppField name="min_swarm_seeders_to_seed">
            {(field) => (
              <field.NumberRow
                label="Min swarm seeders"
                hint="0 = off · pause when scrape < N"
                min={0}
              />
            )}
          </group.AppField>
        </Section>

        {!hideCeiling && (
          <group.Subscribe selector={(s) => s.values}>
            {(policy) => <AggregateCeiling policy={policy} />}
          </group.Subscribe>
        )}

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
            <group.AppField name="pause_torrent_with_zero_leechers">
              {(field) => (
                <field.CheckboxRow
                  label="Pause on zero leechers"
                  hint="auto-pause when swarm has no leechers"
                />
              )}
            </group.AppField>
            <group.AppField name="pause_torrent_with_zero_leechers_grace">
              {(field) => (
                <field.NumberRow
                  label="Zero-leechers grace"
                  hint="seconds before pause fires"
                  min={0}
                />
              )}
            </group.AppField>
            <group.AppField name="max_announce_jitter">
              {(field) => (
                <field.NumberRow
                  label="Announce jitter"
                  hint="0–N s drift on reschedule"
                  min={0}
                />
              )}
            </group.AppField>
          </Section>
        )}
      </div>
    );
  },
});

/* ──────────────────────────── Sections ──────────────────────────── */

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

function RangeRow({
  label,
  renderMin,
  renderMax,
}: {
  label: string;
  renderMin: () => React.ReactNode;
  renderMax: () => React.ReactNode;
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
        {renderMin()}
        <span
          aria-hidden="true"
          className="font-mono text-[11px] text-muted-foreground/60"
        >
          —
        </span>
        {renderMax()}
      </div>
    </div>
  );
}

/* ────────────────────────── Aggregate ceiling ───────────────────────── */

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
