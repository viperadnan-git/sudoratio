import { useQueryClient } from "@tanstack/react-query";
import { createFileRoute, useNavigate } from "@tanstack/react-router";
import {
  AlertTriangle,
  CheckCircle2,
  Cloud,
  Cog,
  Copy,
  Loader2,
  RotateCcw,
  Save,
  Sparkles,
  Trash2,
  Wifi,
  X,
  XCircle,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { toast } from "sonner";
import { z } from "zod";

import type { DiffListItem } from "@/components/diff-list";
import { NewChip, PresetChip } from "@/components/preset-chip";
import { PresetDeleteDialog } from "@/components/preset-delete-dialog";
import { PresetPolicyFields } from "@/components/preset-policy-fields";
import { SaveConfirmDialog } from "@/components/save-confirm-dialog";
import { Button } from "@/components/ui/button";
import { useAppForm } from "@/lib/form-hook";
import { PRESET_SWATCHES } from "@/lib/preset-colors";
import {
  fetchConfigDefaults,
  fetchPresetDefaults,
  useCheckConnectivity,
  useConfig,
  useCreatePreset,
  usePresets,
  useUpdateConfig,
  useUpdatePreset,
} from "@/lib/queries";
import {
  type ConfigBody,
  type ConfigUpdate,
  configBodySchema,
  DEFAULT_POLICY,
  type PresetForm,
  type PresetPolicy,
  presetFormSchema,
} from "@/lib/schemas";
import type { ConnectivityFamily, Preset } from "@/lib/types";
import { cn } from "@/lib/utils";

const searchSchema = z.object({
  preset: z.string().optional().catch(undefined),
  new: z.literal("1").optional().catch(undefined),
  clone: z.string().optional().catch(undefined),
});

export const Route = createFileRoute("/_authed/config")({
  validateSearch: searchSchema,
  component: ConfigPage,
});

type Tab =
  | { kind: "engine" }
  | { kind: "preset"; id: string }
  | { kind: "draft"; cloneFrom?: string };

function ConfigPage() {
  const navigate = useNavigate({ from: "/config" });
  const search = Route.useSearch();
  const { data: presets } = usePresets();
  const [deletePreset, setDeletePreset] = useState<Preset | null>(null);

  const tab: Tab = useMemo(() => {
    if (search.clone) return { kind: "draft", cloneFrom: search.clone };
    if (search.new === "1") return { kind: "draft" };
    if (search.preset) return { kind: "preset", id: search.preset };
    return { kind: "engine" };
  }, [search]);

  // If a deleted preset was active, fall back to engine.
  useEffect(() => {
    if (tab.kind === "preset" && presets) {
      if (!presets.some((p) => p.id === tab.id)) {
        navigate({ search: () => ({}) });
      }
    }
  }, [presets, tab, navigate]);

  const setTab = (next: Tab) => {
    navigate({
      search: () => {
        if (next.kind === "engine") return {};
        if (next.kind === "draft") {
          return next.cloneFrom ? { clone: next.cloneFrom } : { new: "1" };
        }
        return { preset: next.id };
      },
    });
  };

  return (
    <div className="px-3 pb-12 pt-4 md:px-6 md:pt-6">
      <header className="mb-4 flex flex-col items-start gap-2 md:mb-6 md:flex-row md:items-end md:justify-between md:gap-4">
        <div>
          <div className="eyebrow mb-1.5">Operations · Config</div>
          <h1 className="text-[22px] font-semibold leading-tight tracking-tight md:text-[28px]">
            Engine & presets
          </h1>
          <p className="mt-1.5 max-w-md font-mono text-[11.5px] text-muted-foreground">
            changes apply live · per-tracker policy lives in presets
          </p>
        </div>
      </header>

      {/* Tab strip — Engine + each preset, plus draft + new */}
      <div className="-mx-3 mb-5 px-3 md:-mx-6 md:px-6">
        <div
          className={cn(
            "flex snap-x snap-mandatory items-center gap-1.5 overflow-x-auto pb-1",
            "[scrollbar-width:none] [&::-webkit-scrollbar]:hidden",
            "md:flex-wrap md:gap-2 md:overflow-x-visible md:pb-0",
          )}
        >
          <PresetChip
            active={tab.kind === "engine"}
            onSelect={() => setTab({ kind: "engine" })}
            label="Engine"
            color="var(--foreground)"
            icon={<Cog className="size-3" strokeWidth={2} />}
          />
          {(presets ?? []).map((p) => (
            <PresetChip
              key={p.id}
              active={tab.kind === "preset" && tab.id === p.id}
              onSelect={() => setTab({ kind: "preset", id: p.id })}
              label={p.name}
              color={p.color}
            />
          ))}
          {tab.kind === "draft" && (
            <PresetChip
              active
              onSelect={() => {}}
              label="New preset"
              color="var(--muted-foreground)"
              icon={<Sparkles className="size-3" strokeWidth={2} />}
            />
          )}
          {tab.kind !== "draft" && (
            <NewChip onClick={() => setTab({ kind: "draft" })} />
          )}
        </div>
      </div>

      {tab.kind === "engine" && <EnginePanel />}
      {tab.kind === "preset" && (
        <PresetEditor
          key={`edit:${tab.id}`}
          mode={{ kind: "edit", id: tab.id }}
          onDelete={() => {
            const p = (presets ?? []).find((x) => x.id === tab.id);
            if (p) setDeletePreset(p);
          }}
          onClone={() => setTab({ kind: "draft", cloneFrom: tab.id })}
          onCancel={() => setTab({ kind: "engine" })}
        />
      )}
      {tab.kind === "draft" && (
        <PresetEditor
          key={tab.cloneFrom ? `clone:${tab.cloneFrom}` : "draft"}
          mode={{ kind: "draft", cloneFrom: tab.cloneFrom }}
          onCancel={() => setTab({ kind: "engine" })}
          onCreated={(id) => setTab({ kind: "preset", id })}
        />
      )}

      <PresetDeleteDialog
        open={!!deletePreset}
        onOpenChange={(v) => !v && setDeletePreset(null)}
        preset={deletePreset}
      />
    </div>
  );
}

/* ───────────────────────── PRESET EDITOR (edit + draft) ───────────────────────── */

type EditorMode =
  | { kind: "edit"; id: string }
  | { kind: "draft"; cloneFrom?: string };

function defaultsForMode(
  preset: Preset | null,
  clone: Preset | null,
): PresetForm {
  if (preset) {
    return { name: preset.name, color: preset.color, policy: preset.policy };
  }
  if (clone) {
    return {
      name: `${clone.name} copy`,
      color: clone.color,
      policy: clone.policy,
    };
  }
  return { name: "", color: PRESET_SWATCHES[1].hex, policy: DEFAULT_POLICY };
}

function PresetEditor({
  mode,
  onCancel,
  onDelete,
  onClone,
  onCreated,
}: {
  mode: EditorMode;
  onCancel: () => void;
  onDelete?: () => void;
  onClone?: () => void;
  onCreated?: (id: string) => void;
}) {
  const { data: presets } = usePresets();
  const preset =
    mode.kind === "edit"
      ? ((presets ?? []).find((p) => p.id === mode.id) ?? null)
      : null;
  const cloneSource =
    mode.kind === "draft" && mode.cloneFrom
      ? ((presets ?? []).find((p) => p.id === mode.cloneFrom) ?? null)
      : null;

  const updateMut = useUpdatePreset();
  const createMut = useCreatePreset();
  const qc = useQueryClient();
  const [resetting, setResetting] = useState(false);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const isDraft = mode.kind === "draft";
  const busy = updateMut.isPending || createMut.isPending || resetting;

  const form = useAppForm({
    defaultValues: defaultsForMode(preset, cloneSource),
    validators: { onChange: presetFormSchema },
    onSubmit: async ({ value }) => {
      if (isDraft) {
        try {
          const created = await createMut.mutateAsync(value);
          toast.success(`Preset "${created.name}" created`);
          onCreated?.(created.id);
        } catch (e) {
          toast.error(e instanceof Error ? e.message : "create failed");
        }
        return;
      }
      if (!preset) return;
      try {
        await updateMut.mutateAsync({
          id: preset.id,
          patch: {
            name: value.name !== preset.name ? value.name : undefined,
            color:
              value.color.toLowerCase() !== preset.color.toLowerCase()
                ? value.color
                : undefined,
            policy:
              JSON.stringify(value.policy) !== JSON.stringify(preset.policy)
                ? value.policy
                : undefined,
          },
        });
        toast.success(`Preset "${value.name}" saved`);
        setConfirmOpen(false);
      } catch (e) {
        toast.error(e instanceof Error ? e.message : "save failed");
      }
    },
  });

  useEffect(() => {
    if (mode.kind === "edit" && preset) {
      form.reset(defaultsForMode(preset, null));
    } else if (mode.kind === "draft" && cloneSource) {
      form.reset(defaultsForMode(null, cloneSource));
    }
  }, [preset?.id, cloneSource?.id]);

  const onResetDefaults = async () => {
    setResetting(true);
    try {
      const defaults = await fetchPresetDefaults(qc);
      form.setFieldValue("policy", defaults);
      toast.message("Defaults loaded · review and save");
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "fetch defaults failed");
    } finally {
      setResetting(false);
    }
  };

  if (mode.kind === "edit" && !preset) {
    return (
      <div className="px-3 py-6 font-mono text-[12px] text-muted-foreground">
        › loading preset…
      </div>
    );
  }

  return (
    <form
      className="space-y-4"
      onSubmit={(e) => {
        e.preventDefault();
        form.handleSubmit();
      }}
    >
      {/* Identity card with inline name + color picker */}
      <section className="overflow-hidden rounded-md border bg-card">
        <div className="flex items-center justify-between gap-3 px-3 py-3 md:px-4">
          <div className="flex min-w-0 flex-1 items-center gap-2.5">
            <form.AppField name="color">
              {(field) => <field.ColorPickerField disabled={busy} />}
            </form.AppField>
            <div className="min-w-0 flex-1">
              <form.AppField name="name">
                {(field) => (
                  <field.InlineEditField
                    placeholder={isDraft ? "Preset name" : "click to rename"}
                    disabled={busy}
                    startInEditMode={isDraft}
                    ariaLabel="Edit preset name"
                    className="text-[14px] font-semibold leading-tight md:text-[15px]"
                  />
                )}
              </form.AppField>
              <form.Subscribe selector={(s) => s.values.name}>
                {(name) => (
                  <div className="mt-0.5 flex items-center gap-1.5 font-mono text-[10.5px] uppercase tracking-wider text-muted-foreground/70">
                    <span>
                      #
                      {isDraft
                        ? slugifyName(name) || "preset"
                        : (preset?.id ?? "")}
                    </span>
                    {!isDraft && preset?.is_default && (
                      <>
                        <span className="text-muted-foreground/40">·</span>
                        <span>default</span>
                      </>
                    )}
                    {isDraft && (
                      <>
                        <span className="text-muted-foreground/40">·</span>
                        <span className="text-muted-foreground/55">draft</span>
                      </>
                    )}
                  </div>
                )}
              </form.Subscribe>
            </div>
          </div>
          <div className="flex shrink-0 items-center gap-1.5">
            <form.Subscribe
              selector={(s) => ({
                isValid: s.isValid,
                isDirty: s.isDirty,
                values: s.values,
              })}
            >
              {({ isValid, isDirty, values }) => {
                const editDiff: DiffListItem[] = preset
                  ? buildPresetDiff(preset, values)
                  : [];
                if (isDraft) {
                  return (
                    <div
                      key="draft-actions"
                      className="flex items-center gap-1.5 animate-in fade-in-0 slide-in-from-right-2 duration-200 ease-out"
                    >
                      <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        className="h-8 gap-1.5 px-2.5 text-[12px]"
                        onClick={onCancel}
                        disabled={busy}
                      >
                        <X className="size-3.5" strokeWidth={2} />
                        <span className="hidden md:inline">Cancel</span>
                      </Button>
                      <Button
                        type="submit"
                        size="sm"
                        className="h-8 gap-1.5 px-3 text-[12px]"
                        disabled={!isValid || busy}
                      >
                        <Save className="size-3.5" strokeWidth={2} />
                        {createMut.isPending ? "Creating…" : "Create"}
                      </Button>
                    </div>
                  );
                }
                if (isDirty) {
                  return (
                    <div
                      key="dirty-actions"
                      className="flex items-center gap-1.5 animate-in fade-in-0 slide-in-from-right-2 duration-200 ease-out"
                    >
                      <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        className="h-8 gap-1.5 px-2.5 text-[12px]"
                        onClick={() => form.reset()}
                        disabled={busy}
                      >
                        <X className="size-3.5" strokeWidth={2} />
                        <span className="hidden md:inline">Discard</span>
                      </Button>
                      <Button
                        type="button"
                        size="sm"
                        className="h-8 gap-1.5 px-3 text-[12px]"
                        onClick={() => setConfirmOpen(true)}
                        disabled={!isValid || busy}
                      >
                        <Save className="size-3.5" strokeWidth={2} />
                        {updateMut.isPending ? "Saving…" : "Save"}
                        {editDiff.length > 0 && (
                          <span className="ml-0.5 rounded-full bg-foreground/15 px-1.5 font-mono text-[10px] tabular-nums leading-none py-[3px]">
                            {editDiff.length}
                          </span>
                        )}
                      </Button>
                    </div>
                  );
                }
                if (preset && !preset.is_default) {
                  return (
                    <div
                      key="clean-actions"
                      className="flex items-center gap-1.5 animate-in fade-in-0 slide-in-from-right-2 duration-200 ease-out"
                    >
                      <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        className="h-8 gap-1.5 px-2.5 text-[12px]"
                        onClick={onClone}
                        title="Create a new preset prefilled with these values"
                      >
                        <Copy className="size-3.5" strokeWidth={2} />
                        <span className="hidden md:inline">Clone</span>
                      </Button>
                      <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        className="h-8 gap-1.5 px-2.5 text-[12px] text-destructive hover:bg-destructive/10 hover:text-destructive"
                        onClick={onDelete}
                      >
                        <Trash2 className="size-3.5" strokeWidth={2} />
                        <span className="hidden md:inline">Delete</span>
                      </Button>
                    </div>
                  );
                }
                if (preset?.is_default) {
                  return (
                    <div
                      key="default-clean-actions"
                      className="flex items-center gap-1.5 animate-in fade-in-0 slide-in-from-right-2 duration-200 ease-out"
                    >
                      <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        className="h-8 gap-1.5 px-2.5 text-[12px]"
                        onClick={onClone}
                        title="Create a new preset prefilled with these values"
                      >
                        <Copy className="size-3.5" strokeWidth={2} />
                        <span className="hidden md:inline">Clone</span>
                      </Button>
                      <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        className="h-8 gap-1.5 px-2.5 text-[12px]"
                        onClick={onResetDefaults}
                        disabled={busy}
                        title="Load compile-time default policy"
                      >
                        {resetting ? (
                          <Loader2
                            className="size-3.5 animate-spin"
                            strokeWidth={2}
                          />
                        ) : (
                          <RotateCcw className="size-3.5" strokeWidth={2} />
                        )}
                        <span className="hidden md:inline">Reset</span>
                      </Button>
                    </div>
                  );
                }
                return null;
              }}
            </form.Subscribe>
          </div>
        </div>
      </section>

      <PresetPolicyFields form={form} fields="policy" hideCeiling={false} />

      {!isDraft && preset && (
        <form.Subscribe selector={(s) => s.values}>
          {(values) => (
            <SaveConfirmDialog
              open={confirmOpen}
              onOpenChange={setConfirmOpen}
              eyebrow={`Preset · ${preset.name}`}
              title="Apply preset changes?"
              description="Changes apply live to all torrents in this preset on the next bandwidth tick."
              items={buildPresetDiff(preset, values)}
              pending={updateMut.isPending}
              confirmLabel="Save preset"
              onConfirm={() => form.handleSubmit()}
            />
          )}
        </form.Subscribe>
      )}
    </form>
  );
}

function buildPresetDiff(preset: Preset, value: PresetForm): DiffListItem[] {
  const items: DiffListItem[] = [];
  if (value.name !== preset.name)
    items.push({ key: "name", from: preset.name, to: value.name });
  if (value.color.toLowerCase() !== preset.color.toLowerCase())
    items.push({ key: "color", from: preset.color, to: value.color });
  items.push(...diffPolicy(preset.policy, value.policy));
  return items;
}

function slugifyName(s: string): string {
  let out = "";
  let prevDash = false;
  for (const c of s) {
    const lc = c.toLowerCase();
    if (/[a-z0-9]/.test(lc)) {
      out += lc;
      prevDash = false;
    } else if (!prevDash && out.length > 0) {
      out += "-";
      prevDash = true;
    }
  }
  while (out.endsWith("-")) out = out.slice(0, -1);
  return out.slice(0, 32);
}

/* ───────────────────────── ENGINE PANEL ───────────────────────── */

// Split into shell + form so `useAppForm` only mounts with a real
// `defaultValues` — initialising it with an empty object would lock that
// in as the dirty-baseline.
function EnginePanel() {
  const cfg = useConfig();
  if (!cfg.data) {
    return (
      <div className="px-3 py-6 font-mono text-[12px] text-muted-foreground">
        {cfg.isLoading ? "› loading config…" : "› no config available"}
      </div>
    );
  }
  return <EnginePanelForm initial={cfg.data} />;
}

function EnginePanelForm({ initial }: { initial: ConfigBody }) {
  const update = useUpdateConfig();
  const qc = useQueryClient();
  const [resetting, setResetting] = useState(false);
  const [confirmOpen, setConfirmOpen] = useState(false);

  const form = useAppForm({
    defaultValues: initial,
    validators: { onChange: configBodySchema },
    onSubmit: async ({ value }) => {
      const patch = diffConfig(initial, value);
      if (Object.keys(patch).length === 0) {
        toast.message("Nothing changed");
        setConfirmOpen(false);
        return;
      }
      try {
        const next = await update.mutateAsync(patch);
        form.reset(next);
        toast.success("Engine config saved");
        setConfirmOpen(false);
      } catch (e) {
        toast.error(e instanceof Error ? e.message : "save failed");
      }
    },
  });

  const onReset = async () => {
    setResetting(true);
    try {
      const defaults = await fetchConfigDefaults(qc);
      form.reset(defaults);
      toast.message("Defaults loaded · review and save");
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "fetch defaults failed");
    } finally {
      setResetting(false);
    }
  };

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        form.handleSubmit();
      }}
    >
      <form.Subscribe
        selector={(s) => ({
          isDirty: s.isDirty,
          isValid: s.isValid,
          values: s.values,
        })}
      >
        {({ isDirty, isValid, values }) => {
          const engineDiff = diffConfigList(initial, values);
          return (
            <div
              aria-hidden={!isDirty}
              className={cn(
                "grid transition-[grid-template-rows,margin-bottom] duration-200 ease-out",
                isDirty ? "mb-4 grid-rows-[1fr]" : "mb-0 grid-rows-[0fr]",
              )}
            >
              <div className="min-h-0 overflow-hidden">
                <div
                  className={cn(
                    "flex items-center gap-2 rounded-md border border-amber-500/30 bg-amber-500/[0.04] p-2 shadow-sm transition-opacity duration-200 ease-out",
                    isDirty ? "opacity-100" : "pointer-events-none opacity-0",
                  )}
                >
                  <span
                    aria-hidden="true"
                    className="ml-1 size-1.5 shrink-0 rounded-full bg-amber-500"
                  />
                  <span className="min-w-0 flex-1 truncate font-mono text-[10.5px] uppercase tracking-wider text-amber-700 dark:text-amber-400">
                    {engineDiff.length} unsaved change
                    {engineDiff.length === 1 ? "" : "s"}
                  </span>
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    className="h-8 gap-1.5 px-2.5 text-[12px]"
                    onClick={() => form.reset()}
                    disabled={!isDirty || update.isPending}
                    tabIndex={isDirty ? 0 : -1}
                  >
                    <X className="size-3.5" strokeWidth={2} />
                    Discard
                  </Button>
                  <Button
                    type="button"
                    size="sm"
                    className="h-8 gap-1.5 px-3 text-[12px]"
                    onClick={() => setConfirmOpen(true)}
                    disabled={!isDirty || !isValid || update.isPending}
                    tabIndex={isDirty ? 0 : -1}
                  >
                    <Save className="size-3.5" strokeWidth={2} />
                    {update.isPending ? "Saving…" : "Review & save"}
                  </Button>
                </div>
              </div>
            </div>
          );
        }}
      </form.Subscribe>

      <div className="space-y-4 md:columns-2 md:gap-4 md:space-y-0 [&>*]:mb-4 [&>*]:break-inside-avoid">
        <Panel
          icon={Wifi}
          title="Network"
          description="announce port + concurrency caps"
        >
          <form.AppField name="announce_port">
            {(field) => (
              <field.NullableNumberRow
                label="Announce port"
                hint="empty = use bound peer-port"
                min={1}
                max={65535}
                placeholder="auto"
              />
            )}
          </form.AppField>
          <form.AppField name="max_concurrent_announces">
            {(field) => (
              <field.NumberRow
                label="Max concurrent announces"
                hint="0 = unlimited"
                min={0}
              />
            )}
          </form.AppField>
          <form.AppField name="bandwidth_tick_ms">
            {(field) => (
              <field.NumberRow
                label="Bandwidth tick"
                hint="ms · simulator interval"
                min={1}
              />
            )}
          </form.AppField>
          <form.Subscribe selector={(s) => s.values.announce_port}>
            {(port) => <ConnectivityRow port={port ?? null} />}
          </form.Subscribe>
        </Panel>

        <Panel
          icon={Cloud}
          title="HTTP tracker client"
          description="reqwest knobs · null = library default"
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
              ["http_tracker_max_idle_per_host", "Max idle per host", null],
              ["http_tracker_max_redirects", "Max redirects", null],
              ["http_tracker_tcp_keepalive_secs", "TCP keepalive", "seconds"],
              [
                "http_tracker_pool_idle_timeout_secs",
                "Pool idle timeout",
                "seconds",
              ],
            ] as const
          ).map(([key, label, hint]) => (
            <form.AppField key={key} name={key}>
              {(field) => (
                <field.NullableNumberRow
                  label={label}
                  hint={hint ?? undefined}
                  min={0}
                  placeholder="auto"
                />
              )}
            </form.AppField>
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
                Reset engine to compile-time defaults
              </div>
              <div className="mt-0.5 font-mono text-[10.5px] uppercase tracking-wider text-muted-foreground/70">
                loads defaults · review and save
              </div>
            </div>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="h-8 gap-1.5 px-3 text-[12px] text-destructive hover:bg-destructive/10 hover:text-destructive"
              onClick={onReset}
              disabled={resetting}
            >
              {resetting ? (
                <Loader2 className="size-3.5 animate-spin" strokeWidth={2} />
              ) : (
                <RotateCcw className="size-3.5" strokeWidth={2} />
              )}
              Reset
            </Button>
          </div>
        </section>
      </div>

      <form.Subscribe selector={(s) => s.values}>
        {(values) => (
          <SaveConfirmDialog
            open={confirmOpen}
            onOpenChange={setConfirmOpen}
            eyebrow="Engine"
            title="Apply engine changes?"
            description="Engine infra knobs apply live. HTTP-tracker pool changes take effect on the next outgoing announce."
            items={diffConfigList(initial, values)}
            pending={update.isPending}
            confirmLabel="Save engine"
            onConfirm={() => form.handleSubmit()}
          />
        )}
      </form.Subscribe>
    </form>
  );
}

/* ───────────────────────── HELPERS ───────────────────────── */

function diffConfig(prev: ConfigBody, next: ConfigBody): ConfigUpdate {
  const out: ConfigUpdate = {};
  for (const k of Object.keys(next) as (keyof ConfigBody)[]) {
    if (next[k] !== prev[k]) {
      // @ts-expect-error wide assign across union
      out[k] = next[k];
    }
  }
  return out;
}

function Panel({
  icon: Icon,
  title,
  description,
  children,
}: {
  icon: typeof Wifi;
  title: string;
  description?: string;
  children: React.ReactNode;
}) {
  return (
    <section className="overflow-hidden rounded-md border bg-card">
      <header className="flex items-center justify-between gap-2 border-b border-border/60 px-4 py-2.5 md:px-5">
        <div className="flex items-center gap-2">
          <span
            className="inline-flex size-5 items-center justify-center rounded-sm bg-foreground/[0.06] text-foreground/70"
            aria-hidden="true"
          >
            <Icon className="size-3" strokeWidth={1.75} />
          </span>
          <h2 className="text-[13px] font-semibold leading-none tracking-tight">
            {title}
          </h2>
        </div>
        {description && (
          <span className="hidden font-mono text-[10px] uppercase tracking-wider text-muted-foreground/65 md:inline">
            {description}
          </span>
        )}
      </header>
      <div className="divide-y divide-border/40 bg-background">{children}</div>
    </section>
  );
}

/* ---- Connectivity check ---- */

function ConnectivityRow({ port }: { port: number | null }) {
  const check = useCheckConnectivity();
  const onClick = async () => {
    try {
      const res = await check.mutateAsync(port ?? undefined);
      const v4 = res.ipv4.reachable ? "ok" : "fail";
      const v6 = res.ipv6.reachable ? "ok" : "fail";
      toast.message(`port ${res.port} · v4 ${v4} · v6 ${v6}`);
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "check failed");
    }
  };
  const r = check.data;
  return (
    <div className="flex flex-col gap-1.5 px-4 py-2.5 md:px-5">
      <div className="flex items-center justify-between gap-3">
        <div className="min-w-0 flex-1">
          <div className="text-[12.5px] font-medium leading-tight">
            Connectivity check
          </div>
          <div className="mt-0.5 font-mono text-[10px] uppercase tracking-wider text-muted-foreground/65">
            tests configured port · v4 + v6
          </div>
        </div>
        <Button
          size="sm"
          variant="ghost"
          className="h-7 gap-1.5 px-2.5 text-[12px]"
          onClick={onClick}
          disabled={check.isPending}
        >
          {check.isPending ? (
            <Loader2 className="size-3.5 animate-spin" strokeWidth={2} />
          ) : (
            <Wifi className="size-3.5" strokeWidth={2} />
          )}
          Test
        </Button>
      </div>
      {r && (
        <div className="grid grid-cols-2 gap-1.5 pt-1">
          <FamilyLine label="IPv4" data={r.ipv4} />
          <FamilyLine label="IPv6" data={r.ipv6} />
        </div>
      )}
    </div>
  );
}

function FamilyLine({
  label,
  data,
}: {
  label: string;
  data: ConnectivityFamily;
}) {
  return (
    <div className="flex items-center gap-1.5 rounded-md border border-border/60 bg-muted/30 px-2 py-1.5 font-mono text-[10.5px]">
      {data.reachable ? (
        <CheckCircle2 className="size-3 text-success" strokeWidth={2} />
      ) : (
        <XCircle className="size-3 text-destructive" strokeWidth={2} />
      )}
      <span className="font-medium uppercase tracking-wider">{label}</span>
      <span className="ml-auto truncate text-muted-foreground/70">
        {data.public_ip ?? data.error ?? "—"}
      </span>
    </div>
  );
}

/* ---- Diff helpers (review-and-save dialogs) ---- */

function fmtVal(v: unknown): string {
  if (v === null || v === undefined) return "—";
  if (typeof v === "boolean") return v ? "on" : "off";
  return String(v);
}

function diffConfigList(prev: ConfigBody, next: ConfigBody): DiffListItem[] {
  const out: DiffListItem[] = [];
  for (const k of Object.keys(next) as (keyof ConfigBody)[]) {
    if (next[k] !== prev[k]) {
      out.push({ key: k, from: fmtVal(prev[k]), to: fmtVal(next[k]) });
    }
  }
  return out;
}

function diffPolicy(prev: PresetPolicy, next: PresetPolicy): DiffListItem[] {
  const out: DiffListItem[] = [];
  for (const k of Object.keys(next) as (keyof PresetPolicy)[]) {
    if (next[k] !== prev[k]) {
      out.push({ key: k, from: fmtVal(prev[k]), to: fmtVal(next[k]) });
    }
  }
  return out;
}
