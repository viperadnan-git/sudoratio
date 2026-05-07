import { useQueryClient } from "@tanstack/react-query";
import { createFileRoute, useNavigate } from "@tanstack/react-router";
import {
  AlertTriangle,
  CheckCircle2,
  Cloud,
  Cog,
  Loader2,
  Plus,
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
import { PresetDeleteDialog } from "@/components/preset-delete-dialog";
import { PresetPolicyFields } from "@/components/preset-policy-fields";
import { SaveConfirmDialog } from "@/components/save-confirm-dialog";
import { Button } from "@/components/ui/button";
import { InlineEdit } from "@/components/ui/inline-edit";
import { Input } from "@/components/ui/input";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import {
  isHexColor,
  PRESET_SWATCHES,
  tintBackground,
  tintBorder,
} from "@/lib/preset-colors";
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
import type {
  ConfigBody,
  ConfigUpdate,
  ConnectivityFamily,
  Preset,
  PresetPolicy,
} from "@/lib/types";
import { cn } from "@/lib/utils";

const searchSchema = z.object({
  preset: z.string().optional().catch(undefined),
  new: z.literal("1").optional().catch(undefined),
});

export const Route = createFileRoute("/_authed/config")({
  validateSearch: searchSchema,
  component: ConfigPage,
});

const DEFAULT_POLICY: PresetPolicy = {
  min_upload_speed: 27,
  max_upload_speed: 183,
  min_download_speed: 800,
  max_download_speed: 1200,
  max_active_torrents: 5,
  upload_ratio_target: 3.0,
  pause_torrent_with_zero_leechers: false,
  pause_torrent_with_zero_leechers_grace: 10800,
  min_swarm_seeders_to_seed: 0,
  max_announce_jitter: 8,
  client_profile_id: null,
};

type Tab =
  | { kind: "engine" }
  | { kind: "preset"; id: string }
  | { kind: "draft" };

function ConfigPage() {
  const navigate = useNavigate({ from: "/config" });
  const search = Route.useSearch();
  const { data: presets } = usePresets();
  const [deletePreset, setDeletePreset] = useState<Preset | null>(null);

  const tab: Tab = useMemo(() => {
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
        if (next.kind === "draft") return { new: "1" };
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
          <TabChip
            active={tab.kind === "engine"}
            onSelect={() => setTab({ kind: "engine" })}
            label="Engine"
            color="var(--foreground)"
            icon={<Cog className="size-3" strokeWidth={2} />}
          />
          {(presets ?? []).map((p) => (
            <TabChip
              key={p.id}
              active={tab.kind === "preset" && tab.id === p.id}
              onSelect={() => setTab({ kind: "preset", id: p.id })}
              label={p.name}
              color={p.color}
            />
          ))}
          {tab.kind === "draft" && (
            <TabChip
              active
              onSelect={() => {}}
              label="New preset"
              color="var(--muted-foreground)"
              icon={<Sparkles className="size-3" strokeWidth={2} />}
            />
          )}
          {tab.kind !== "draft" && (
            <button
              type="button"
              onClick={() => setTab({ kind: "draft" })}
              className={cn(
                "inline-flex shrink-0 cursor-pointer items-center gap-1 rounded-full border border-dashed border-border/80 px-3 py-1.5 text-muted-foreground transition-colors hover:border-foreground/40 hover:text-foreground",
              )}
            >
              <Plus className="size-3" strokeWidth={2} />
              <span className="font-mono text-[10.5px] uppercase tracking-wider">
                new
              </span>
            </button>
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
          onCancel={() => setTab({ kind: "engine" })}
        />
      )}
      {tab.kind === "draft" && (
        <PresetEditor
          key="draft"
          mode={{ kind: "draft" }}
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

function TabChip({
  active,
  onSelect,
  label,
  color,
  icon,
}: {
  active: boolean;
  onSelect: () => void;
  label: string;
  color: string;
  icon?: React.ReactNode;
}) {
  // For non-color colors (CSS vars on Engine/Draft tabs), the tint helpers
  // can't compute rgba(...). Fall back to a neutral foreground tint.
  const isHex = isHexColor(color);
  const styleVars = isHex
    ? ({
        "--chip-bg": tintBackground(color, 0.08),
        "--chip-border": tintBorder(color),
      } as React.CSSProperties)
    : ({
        "--chip-bg": "rgba(127, 127, 127, 0.08)",
        "--chip-border": "rgba(127, 127, 127, 0.35)",
      } as React.CSSProperties);

  return (
    <button
      type="button"
      onClick={onSelect}
      data-active={active}
      className={cn(
        "group relative inline-flex shrink-0 snap-start cursor-pointer items-center gap-2 rounded-full border px-3 py-1.5 transition-all md:px-3.5 md:py-1.5",
        "border-border/70 bg-card hover:bg-foreground/[0.04]",
        "data-[active=true]:border-[color:var(--chip-border)] data-[active=true]:bg-[color:var(--chip-bg)] data-[active=true]:shadow-[inset_0_0_0_1px_var(--chip-border)]",
      )}
      style={styleVars}
    >
      {icon ? (
        <span
          className={cn(
            "transition-colors",
            active ? "text-foreground" : "text-foreground/70",
          )}
        >
          {icon}
        </span>
      ) : (
        <span
          aria-hidden="true"
          className={cn(
            "size-2 shrink-0 rounded-full transition-transform",
            active && "scale-110",
          )}
          style={{ background: color }}
        />
      )}
      <span
        className={cn(
          "text-[12.5px] font-medium leading-none tracking-tight transition-colors",
          active ? "text-foreground" : "text-foreground/80",
        )}
      >
        {label}
      </span>
    </button>
  );
}

/* ───────────────────────── PRESET EDITOR (edit + draft) ───────────────────────── */

type EditorMode = { kind: "edit"; id: string } | { kind: "draft" };

function PresetEditor({
  mode,
  onCancel,
  onDelete,
  onCreated,
}: {
  mode: EditorMode;
  onCancel: () => void;
  onDelete?: () => void;
  onCreated?: (id: string) => void;
}) {
  const { data: presets } = usePresets();
  const preset =
    mode.kind === "edit"
      ? ((presets ?? []).find((p) => p.id === mode.id) ?? null)
      : null;

  const updateMut = useUpdatePreset();
  const createMut = useCreatePreset();
  const qc = useQueryClient();
  const [resetting, setResetting] = useState(false);
  const busy = updateMut.isPending || createMut.isPending || resetting;

  const [name, setName] = useState(preset?.name ?? "");
  const [color, setColor] = useState(preset?.color ?? PRESET_SWATCHES[1].hex);
  const [policy, setPolicy] = useState<PresetPolicy>(
    preset?.policy ?? DEFAULT_POLICY,
  );
  const [confirmOpen, setConfirmOpen] = useState(false);

  // Re-seed when the bound preset changes (e.g. switching tabs).
  useEffect(() => {
    if (mode.kind === "edit" && preset) {
      setName(preset.name);
      setColor(preset.color);
      setPolicy(preset.policy);
    }
    if (mode.kind === "draft") {
      setName("");
      setColor(PRESET_SWATCHES[1].hex);
      setPolicy(DEFAULT_POLICY);
    }
  }, [mode, preset]);

  const isDraft = mode.kind === "draft";
  const valid = name.trim().length > 0 && isHexColor(color);

  const editDirty =
    !isDraft &&
    !!preset &&
    (name !== preset.name ||
      color.toLowerCase() !== preset.color.toLowerCase() ||
      JSON.stringify(policy) !== JSON.stringify(preset.policy));

  const draftDirty =
    isDraft &&
    (name !== "" ||
      color.toLowerCase() !== PRESET_SWATCHES[1].hex ||
      JSON.stringify(policy) !== JSON.stringify(DEFAULT_POLICY));

  const dirty = isDraft ? draftDirty : editDirty;

  // Build edit-mode diff for the confirm dialog (policy + meta combined).
  const editDiff = useMemo<DiffListItem[]>(() => {
    if (isDraft || !preset) return [];
    const items: DiffListItem[] = [];
    if (name !== preset.name)
      items.push({ key: "name", from: preset.name, to: name });
    if (color.toLowerCase() !== preset.color.toLowerCase())
      items.push({ key: "color", from: preset.color, to: color });
    items.push(...diffPolicy(preset.policy, policy));
    return items;
  }, [isDraft, preset, name, color, policy]);

  const onClear = () => {
    if (isDraft) {
      setName("");
      setColor(PRESET_SWATCHES[1].hex);
      setPolicy(DEFAULT_POLICY);
      return;
    }
    if (preset) {
      setName(preset.name);
      setColor(preset.color);
      setPolicy(preset.policy);
    }
  };

  const onConfirmEditSave = async () => {
    if (!preset) return;
    try {
      await updateMut.mutateAsync({
        id: preset.id,
        patch: {
          name: name !== preset.name ? name : undefined,
          color:
            color.toLowerCase() !== preset.color.toLowerCase()
              ? color
              : undefined,
          policy:
            JSON.stringify(policy) !== JSON.stringify(preset.policy)
              ? policy
              : undefined,
        },
      });
      toast.success(`Preset "${name}" saved`);
      setConfirmOpen(false);
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "save failed");
    }
  };

  const onResetDefaults = async () => {
    setResetting(true);
    try {
      const defaults = await fetchPresetDefaults(qc);
      setPolicy(defaults);
      toast.message("Defaults loaded · review and save");
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "fetch defaults failed");
    } finally {
      setResetting(false);
    }
  };

  const onCreateClick = async () => {
    try {
      const created = await createMut.mutateAsync({ name, color, policy });
      toast.success(`Preset "${created.name}" created`);
      onCreated?.(created.id);
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "create failed");
    }
  };

  if (mode.kind === "edit" && !preset) {
    return (
      <div className="px-3 py-6 font-mono text-[12px] text-muted-foreground">
        › loading preset…
      </div>
    );
  }

  const slug = isDraft ? slugifyName(name) || "preset" : (preset?.id ?? "");

  return (
    <div className="space-y-4">
      {/* Identity card with inline name + color picker */}
      <section className="overflow-hidden rounded-md border bg-card">
        <div className="flex items-center justify-between gap-3 px-3 py-3 md:px-4">
          <div className="flex min-w-0 flex-1 items-center gap-2.5">
            <ColorPickerButton
              value={color}
              onChange={setColor}
              disabled={busy}
            />
            <div className="min-w-0 flex-1">
              <InlineEdit
                value={name}
                onChange={setName}
                placeholder={isDraft ? "Preset name" : "click to rename"}
                disabled={busy}
                startInEditMode={isDraft}
                ariaLabel="Edit preset name"
                className="text-[14px] font-semibold leading-tight md:text-[15px]"
              />
              <div className="mt-0.5 flex items-center gap-1.5 font-mono text-[10.5px] uppercase tracking-wider text-muted-foreground/70">
                <span>#{slug}</span>
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
            </div>
          </div>
          <div className="flex shrink-0 items-center gap-1.5">
            {isDraft ? (
              <div
                key="draft-actions"
                className="flex items-center gap-1.5 animate-in fade-in-0 slide-in-from-right-2 duration-200 ease-out"
              >
                <Button
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
                  size="sm"
                  className="h-8 gap-1.5 px-3 text-[12px]"
                  onClick={onCreateClick}
                  disabled={!valid || busy}
                >
                  <Save className="size-3.5" strokeWidth={2} />
                  {createMut.isPending ? "Creating…" : "Create"}
                </Button>
              </div>
            ) : dirty ? (
              <div
                key="dirty-actions"
                className="flex items-center gap-1.5 animate-in fade-in-0 slide-in-from-right-2 duration-200 ease-out"
              >
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-8 gap-1.5 px-2.5 text-[12px]"
                  onClick={onClear}
                  disabled={busy}
                >
                  <X className="size-3.5" strokeWidth={2} />
                  <span className="hidden md:inline">Discard</span>
                </Button>
                <Button
                  size="sm"
                  className="h-8 gap-1.5 px-3 text-[12px]"
                  onClick={() => setConfirmOpen(true)}
                  disabled={!valid || busy}
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
            ) : preset && !preset.is_default ? (
              <div
                key="delete-action"
                className="animate-in fade-in-0 slide-in-from-right-2 duration-200 ease-out"
              >
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-8 gap-1.5 px-2.5 text-[12px] text-destructive hover:bg-destructive/10 hover:text-destructive"
                  onClick={onDelete}
                >
                  <Trash2 className="size-3.5" strokeWidth={2} />
                  <span className="hidden md:inline">Delete</span>
                </Button>
              </div>
            ) : preset && preset.is_default ? (
              <div
                key="reset-action"
                className="animate-in fade-in-0 slide-in-from-right-2 duration-200 ease-out"
              >
                <Button
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
            ) : null}
          </div>
        </div>
      </section>

      <PresetPolicyFields value={policy} onChange={setPolicy} />

      {!isDraft && preset && (
        <SaveConfirmDialog
          open={confirmOpen}
          onOpenChange={setConfirmOpen}
          eyebrow={`Preset · ${preset.name}`}
          title="Apply preset changes?"
          description="Changes apply live to all torrents in this preset on the next bandwidth tick."
          items={editDiff}
          pending={updateMut.isPending}
          confirmLabel="Save preset"
          onConfirm={onConfirmEditSave}
        />
      )}
    </div>
  );
}

/* ---- Inline name field: looks like text, becomes input on focus ---- */

/* ---- Color picker: shadcn Popover with swatch grid + hex input ---- */

function ColorPickerButton({
  value,
  onChange,
  disabled,
}: {
  value: string;
  onChange: (hex: string) => void;
  disabled?: boolean;
}) {
  const [open, setOpen] = useState(false);

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          disabled={disabled}
          aria-label="Change preset color"
          className={cn(
            "relative inline-flex size-7 shrink-0 cursor-pointer items-center justify-center rounded-full ring-1 ring-foreground/15 transition-transform hover:scale-105 focus:outline-2 focus:outline-foreground/30",
          )}
          style={{ background: value }}
        />
      </PopoverTrigger>
      <PopoverContent
        align="start"
        sideOffset={8}
        className="w-56 gap-2 p-2"
      >
        <div className="grid grid-cols-4 gap-1.5">
          {PRESET_SWATCHES.map((sw) => {
            const active = value.toLowerCase() === sw.hex;
            return (
              <button
                key={sw.hex}
                type="button"
                aria-label={sw.label}
                data-active={active}
                onClick={() => {
                  onChange(sw.hex);
                  setOpen(false);
                }}
                className={cn(
                  "relative inline-flex size-9 cursor-pointer items-center justify-center rounded-full ring-1 ring-foreground/10 transition-transform hover:scale-110 data-[active=true]:ring-2 data-[active=true]:ring-foreground/40",
                )}
                style={{ background: sw.hex }}
              >
                {active && (
                  <span className="text-[11px] font-bold text-white drop-shadow">
                    ✓
                  </span>
                )}
              </button>
            );
          })}
          <NativeColorTile value={value} onChange={onChange} />
        </div>
        <div className="border-t pt-2">
          <label
            htmlFor="preset-color-hex"
            className="mb-1 block font-mono text-[10px] uppercase tracking-wider text-muted-foreground/70"
          >
            Custom hex
          </label>
          <Input
            id="preset-color-hex"
            value={value}
            onChange={(e) => onChange(e.currentTarget.value)}
            placeholder="#7c3aed"
            className={cn(
              "h-7 font-mono text-[12px]",
              !isHexColor(value) && "border-destructive/50",
            )}
          />
        </div>
      </PopoverContent>
    </Popover>
  );
}

/** Swatch tile that opens the OS-native color picker via a hidden `<input type="color">`. */
function NativeColorTile({
  value,
  onChange,
}: {
  value: string;
  onChange: (hex: string) => void;
}) {
  // Browsers require a valid `#rrggbb` to seed the picker; fall back to white.
  const seed = isHexColor(value) ? value : "#ffffff";
  const inPalette = PRESET_SWATCHES.some(
    (s) => s.hex === value.toLowerCase(),
  );
  const showPreviewFill = !inPalette && isHexColor(value);
  return (
    <label
      data-active={showPreviewFill}
      title="Custom color"
      aria-label="Custom color (native picker)"
      className={cn(
        "relative inline-flex size-9 cursor-pointer items-center justify-center overflow-hidden rounded-full ring-1 ring-foreground/15 transition-transform hover:scale-110 data-[active=true]:ring-2 data-[active=true]:ring-foreground/40",
      )}
      style={
        showPreviewFill
          ? { background: value }
          : {
              background:
                "conic-gradient(from 180deg, #f43f5e, #f59e0b, #84cc16, #10b981, #0ea5e9, #7c3aed, #ec4899, #f43f5e)",
            }
      }
    >
      {showPreviewFill ? (
        <span className="text-[11px] font-bold text-white drop-shadow">
          ✓
        </span>
      ) : (
        <span
          aria-hidden="true"
          className="size-5 rounded-full bg-background/80 ring-1 ring-foreground/20"
        />
      )}
      <input
        type="color"
        value={seed}
        onChange={(e) => onChange(e.currentTarget.value.toLowerCase())}
        className="absolute inset-0 size-full cursor-pointer opacity-0"
        aria-label="Pick custom color"
      />
    </label>
  );
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

function EnginePanel() {
  const cfg = useConfig();
  const update = useUpdateConfig();
  const qc = useQueryClient();
  const [draft, setDraft] = useState<ConfigBody | null>(null);
  const [resetting, setResetting] = useState(false);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const dirty = !!cfg.data && !!draft && !shallowEq(cfg.data, draft);

  useEffect(() => {
    if (cfg.data && !draft) setDraft(cfg.data);
  }, [cfg.data, draft]);

  const engineDiff = useMemo(
    () => (cfg.data && draft ? diffConfigList(cfg.data, draft) : []),
    [cfg.data, draft],
  );

  if (!draft) {
    return (
      <div className="px-3 py-6 font-mono text-[12px] text-muted-foreground">
        {cfg.isLoading ? "› loading config…" : "› no config available"}
      </div>
    );
  }

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

  const onConfirmSave = async () => {
    if (!cfg.data) return;
    const patch = diffConfig(cfg.data, draft);
    if (Object.keys(patch).length === 0) {
      toast.message("Nothing changed");
      setConfirmOpen(false);
      return;
    }
    try {
      const next = await update.mutateAsync(patch);
      setDraft(next);
      toast.success("Engine config saved");
      setConfirmOpen(false);
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "save failed");
    }
  };

  const onReset = async () => {
    setResetting(true);
    try {
      const defaults = await fetchConfigDefaults(qc);
      setDraft(defaults);
      toast.message("Defaults loaded · review and save");
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "fetch defaults failed");
    } finally {
      setResetting(false);
    }
  };

  return (
    <div>
      <div
        aria-hidden={!dirty}
        className={cn(
          "grid transition-[grid-template-rows,margin-bottom] duration-200 ease-out",
          dirty ? "mb-4 grid-rows-[1fr]" : "mb-0 grid-rows-[0fr]",
        )}
      >
        <div className="min-h-0 overflow-hidden">
          <div
            className={cn(
              "flex items-center gap-2 rounded-md border border-amber-500/30 bg-amber-500/[0.04] p-2 shadow-sm transition-opacity duration-200 ease-out",
              dirty ? "opacity-100" : "pointer-events-none opacity-0",
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
              variant="ghost"
              size="sm"
              className="h-8 gap-1.5 px-2.5 text-[12px]"
              onClick={() => setDraft(cfg.data ?? null)}
              disabled={!dirty || update.isPending}
              tabIndex={dirty ? 0 : -1}
            >
              <X className="size-3.5" strokeWidth={2} />
              Discard
            </Button>
            <Button
              size="sm"
              className="h-8 gap-1.5 px-3 text-[12px]"
              onClick={() => setConfirmOpen(true)}
              disabled={!dirty || update.isPending}
              tabIndex={dirty ? 0 : -1}
            >
              <Save className="size-3.5" strokeWidth={2} />
              {update.isPending ? "Saving…" : "Review & save"}
            </Button>
          </div>
        </div>
      </div>

      <div className="space-y-4 md:columns-2 md:gap-4 md:space-y-0 [&>*]:mb-4 [&>*]:break-inside-avoid">
        <Panel
          icon={Wifi}
          title="Network"
          description="announce port + concurrency caps"
        >
          <Row label="Announce port" hint="empty = use bound peer-port">
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
          <Row label="Bandwidth tick" hint="ms · simulator interval">
            <NumInput
              min={1}
              value={draft.bandwidth_tick_ms}
              onChange={setN("bandwidth_tick_ms")}
            />
          </Row>
          <ConnectivityRow port={draft.announce_port} />
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
            <Row key={key} label={label} hint={hint ?? undefined}>
              <NumInput
                min={0}
                allowEmpty
                placeholder="auto"
                value={draft[key]}
                onChange={setNullable(key)}
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
                Reset engine to compile-time defaults
              </div>
              <div className="mt-0.5 font-mono text-[10.5px] uppercase tracking-wider text-muted-foreground/70">
                loads defaults · review and save
              </div>
            </div>
            <Button
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

      <SaveConfirmDialog
        open={confirmOpen}
        onOpenChange={setConfirmOpen}
        eyebrow="Engine"
        title="Apply engine changes?"
        description="Engine infra knobs apply live. HTTP-tracker pool changes take effect on the next outgoing announce."
        items={engineDiff}
        pending={update.isPending}
        confirmLabel="Save engine"
        onConfirm={onConfirmSave}
      />
    </div>
  );
}

/* ───────────────────────── HELPERS ───────────────────────── */

function shallowEq(a: ConfigBody, b: ConfigBody): boolean {
  return (Object.keys(a) as (keyof ConfigBody)[]).every(
    (k) => a[k] === b[k] || (a[k] == null && b[k] == null),
  );
}

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
    <div className="flex min-h-[3rem] items-center justify-between gap-3 px-4 py-2 md:px-5">
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

function NumInput({
  value,
  onChange,
  min,
  max,
  step,
  allowEmpty,
  placeholder,
}: {
  value: number | null | undefined;
  onChange: (v: number | null) => void;
  min?: number;
  max?: number;
  step?: string;
  allowEmpty?: boolean;
  placeholder?: string;
}) {
  return (
    <Input
      type="number"
      min={min}
      max={max}
      step={step}
      placeholder={placeholder}
      className="h-7 w-24 px-2 text-right font-mono text-[12.5px] tabular-nums"
      value={value == null ? "" : value}
      onChange={(e) => {
        const v = e.currentTarget.value;
        if (v === "") {
          onChange(allowEmpty ? null : 0);
          return;
        }
        const n = Number(v);
        onChange(Number.isFinite(n) ? n : 0);
      }}
    />
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
