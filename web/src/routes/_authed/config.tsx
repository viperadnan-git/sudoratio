import { createFileRoute } from "@tanstack/react-router";
import { Save } from "lucide-react";
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
import { useConfig, useUpdateConfig } from "@/lib/queries";
import type { ConfigBody, ConfigUpdate } from "@/lib/types";
import { cn } from "@/lib/utils";

export const Route = createFileRoute("/_authed/config")({
  component: ConfigPage,
});

function ConfigPage() {
  const cfg = useConfig();
  const update = useUpdateConfig();
  const [draft, setDraft] = useState<ConfigBody | null>(null);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const dirty =
    cfg.data && draft && Object.keys(diff(cfg.data, draft)).length > 0;

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

  const num = (v: string): number => {
    const n = Number(v);
    return Number.isFinite(n) ? n : 0;
  };

  return (
    <form onSubmit={onReview} className="px-3 pb-12 pt-4 md:px-6 md:pt-6">
      <header className="mb-5 flex items-end justify-between gap-4 md:mb-7">
        <div>
          <div className="eyebrow mb-1.5">Operations · Config</div>
          <h1 className="text-[22px] font-semibold leading-tight tracking-tight md:text-[28px]">
            Engine parameters
          </h1>
          <p className="mt-1.5 max-w-md font-mono text-[11.5px] text-muted-foreground">
            changes apply live · no restart required, unless noted
          </p>
        </div>
        <Button
          type="submit"
          size="sm"
          disabled={update.isPending || !dirty}
          className="h-8 gap-1.5 px-3 text-[12px]"
        >
          <Save className="size-3.5" strokeWidth={2} />
          {update.isPending ? "Saving…" : dirty ? "Save · live" : "Saved"}
        </Button>
      </header>

      <div className="space-y-5">
        <Panel
          eyebrow="Section · 01"
          title="Network"
          description="Announce port and concurrency limits."
        >
          <Field label="Announce port">
            <Input
              type="number"
              min={1}
              max={65535}
              value={draft.announce_port}
              onChange={(e) => set("announce_port", num(e.target.value))}
              className="font-mono"
            />
          </Field>
          <Field label="Max concurrent announces" hint="0 = unlimited">
            <Input
              type="number"
              min={0}
              value={draft.max_concurrent_announces}
              onChange={(e) =>
                set("max_concurrent_announces", num(e.target.value))
              }
              className="font-mono"
            />
          </Field>
        </Panel>

        <Panel
          eyebrow="Section · 02"
          title="Bandwidth"
          description="Per-torrent simulated up/down caps in decimal KB/s."
        >
          <Field label="Min upload" hint="KB/s">
            <Input
              type="number"
              min={0}
              value={draft.min_upload_speed}
              onChange={(e) => set("min_upload_speed", num(e.target.value))}
              className="font-mono"
            />
          </Field>
          <Field label="Max upload" hint="KB/s">
            <Input
              type="number"
              min={0}
              value={draft.max_upload_speed}
              onChange={(e) => set("max_upload_speed", num(e.target.value))}
              className="font-mono"
            />
          </Field>
          <Field label="Min download" hint="KB/s">
            <Input
              type="number"
              min={0}
              value={draft.min_download_speed}
              onChange={(e) => set("min_download_speed", num(e.target.value))}
              className="font-mono"
            />
          </Field>
          <Field label="Max download" hint="KB/s">
            <Input
              type="number"
              min={0}
              value={draft.max_download_speed}
              onChange={(e) => set("max_download_speed", num(e.target.value))}
              className="font-mono"
            />
          </Field>
          <Field label="Bandwidth tick" hint="ms" className="md:col-span-2">
            <Input
              type="number"
              min={1}
              value={draft.bandwidth_tick_ms}
              onChange={(e) => set("bandwidth_tick_ms", num(e.target.value))}
              className="font-mono"
            />
          </Field>
        </Panel>

        <Panel
          eyebrow="Section · 03"
          title="Lifecycle"
          description="Active-slot cap, ratio target, eviction policy."
        >
          <Field label="Max active torrents">
            <Input
              type="number"
              min={1}
              value={draft.max_active_torrents}
              onChange={(e) => set("max_active_torrents", num(e.target.value))}
              className="font-mono"
            />
          </Field>
          <Field
            label="Upload ratio target"
            hint="-1 to disable; 1.0 = fully seeded"
          >
            <Input
              type="number"
              step="0.1"
              value={draft.upload_ratio_target}
              onChange={(e) => set("upload_ratio_target", num(e.target.value))}
              className="font-mono"
            />
          </Field>
          <Toggle
            label="Pause torrent with zero leechers"
            hint="when on, auto-pause torrents whose swarm reports no leechers"
            checked={draft.pause_torrent_with_zero_leechers}
            onChange={(v) => set("pause_torrent_with_zero_leechers", v)}
          />
          <Field
            label="Zero-leechers grace"
            hint="seconds to wait before pause; resets if a leecher reappears"
          >
            <Input
              type="number"
              value={draft.pause_torrent_with_zero_leechers_grace}
              onChange={(e) =>
                set(
                  "pause_torrent_with_zero_leechers_grace",
                  num(e.target.value),
                )
              }
              className="font-mono"
            />
          </Field>
        </Panel>

        <Panel
          eyebrow="Section · 04"
          title="Tracker HTTP client"
          description="Optional reqwest knobs · read at startup, restart to apply."
          muted
        >
          {(
            [
              ["http_tracker_connect_timeout_secs", "Connect timeout", "s"],
              ["http_tracker_request_timeout_secs", "Request timeout", "s"],
              ["http_tracker_max_idle_per_host", "Max idle / host", ""],
              ["http_tracker_max_redirects", "Max redirects", ""],
              ["http_tracker_tcp_keepalive_secs", "TCP keepalive", "s"],
              ["http_tracker_pool_idle_timeout_secs", "Pool idle timeout", "s"],
            ] as const
          ).map(([k, label, hint]) => (
            <Field key={k} label={label} hint={hint}>
              <Input
                type="number"
                min={0}
                value={draft[k] ?? ""}
                placeholder="default"
                onChange={(e) => {
                  const v = e.target.value;
                  set(k, v === "" ? null : num(v));
                }}
                className="font-mono"
              />
            </Field>
          ))}
        </Panel>
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

function Panel({
  eyebrow,
  title,
  description,
  children,
  muted,
}: {
  eyebrow: string;
  title: string;
  description?: string;
  children: React.ReactNode;
  muted?: boolean;
}) {
  return (
    <section
      className={cn(
        "overflow-hidden rounded-md border bg-card",
        muted && "bg-card/40",
      )}
    >
      <header className="flex flex-col gap-1 border-b px-4 py-3 md:flex-row md:items-baseline md:justify-between md:gap-6 md:px-5">
        <div>
          <div className="eyebrow mb-1">{eyebrow}</div>
          <h2 className="text-[14px] font-semibold leading-tight">{title}</h2>
        </div>
        {description && (
          <p className="font-mono text-[11px] text-muted-foreground md:text-right">
            {description}
          </p>
        )}
      </header>
      <div className="grid grid-cols-1 gap-px bg-border md:grid-cols-2">
        {children}
      </div>
    </section>
  );
}

function Field({
  label,
  hint,
  children,
  className,
}: {
  label: string;
  hint?: string;
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn("flex flex-col gap-2 bg-background p-4 md:p-5", className)}
    >
      <div className="flex items-baseline justify-between gap-3">
        <span className="eyebrow">{label}</span>
        {hint && (
          <span className="font-mono text-[10px] text-muted-foreground/70">
            {hint}
          </span>
        )}
      </div>
      {children}
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
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      onClick={() => onChange(!checked)}
      className="flex items-center justify-between gap-4 bg-background p-4 text-left transition-colors hover:bg-accent/40 md:p-5"
    >
      <div className="min-w-0">
        <div className="text-[13px] font-medium">{label}</div>
        {hint && (
          <div className="mt-1 font-mono text-[11px] text-muted-foreground">
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

function diff(prev: ConfigBody, next: ConfigBody): ConfigUpdate {
  const out: ConfigUpdate = {};
  for (const k of Object.keys(next) as (keyof ConfigBody)[]) {
    if (prev[k] !== next[k]) {
      (out as Record<string, unknown>)[k] = next[k];
    }
  }
  return out;
}
