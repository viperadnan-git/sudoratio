import { createFileRoute } from "@tanstack/react-router";
import {
  AlertTriangle,
  Check,
  ChevronRight,
  Copy,
  Eye,
  Pencil,
  Plus,
  Trash2,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
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
import { Label } from "@/components/ui/label";
import {
  useActivateVariant,
  useClientSource,
  useDeleteClient,
  useProfiles,
  useRegisterClient,
} from "@/lib/queries";
import type { ClientProfileSummary } from "@/lib/types";
import { cn } from "@/lib/utils";

export const Route = createFileRoute("/_authed/clients")({
  component: ClientsPage,
});

type EditorMode = "create" | "clone" | "edit" | "extend";
interface EditorTarget {
  mode: EditorMode;
  initialToml: string;
  /** Client name whose source seeds the editor (clone/edit); null for create/extend. */
  originalClient: string | null;
  /** Bundled client name being extended (extend mode only). */
  extendingClient?: string;
}

/* ──────────────────────────── page ──────────────────────────── */

function ClientsPage() {
  const profiles = useProfiles();
  const list = profiles.data ?? [];
  const active = list.find((p) => p.active) ?? null;

  const [pendingActivate, setPendingActivate] =
    useState<ClientProfileSummary | null>(null);
  const [pendingDeleteClient, setPendingDeleteClient] = useState<Client | null>(
    null,
  );
  const [viewingClient, setViewingClient] = useState<string | null>(null);
  const [editorTarget, setEditorTarget] = useState<EditorTarget | null>(null);

  const [selectedClient, setSelectedClient] = useState<string | null>(null);

  const clients = useMemo(() => groupByClient(list), [list]);

  useEffect(() => {
    if (!selectedClient && active) setSelectedClient(active.client);
  }, [active, selectedClient]);

  const onCloneFromSource = (client: string, source: string) =>
    setEditorTarget({
      mode: "clone",
      initialToml: source,
      originalClient: client,
    });
  const onEditFromSource = (client: string, source: string) =>
    setEditorTarget({
      mode: "edit",
      initialToml: source,
      originalClient: client,
    });
  const onExtendBundled = (client: string) =>
    setEditorTarget({
      mode: "extend",
      initialToml: extensionTemplate(client),
      originalClient: null,
      extendingClient: client,
    });

  return (
    <div className="px-3 pb-12 pt-4 md:px-6 md:pt-6">
      {/* ── Header ── */}
      <header className="mb-5 flex items-end justify-between gap-4 md:mb-7">
        <div>
          <div className="eyebrow mb-1.5">Operations · Clients</div>
          <h1 className="text-[22px] font-semibold leading-tight tracking-tight md:text-[28px]">
            Emulated clients
          </h1>
          <p className="mt-1.5 max-w-md font-mono text-[11.5px] text-muted-foreground">
            the active variant shapes every announce sent to trackers
          </p>
        </div>
        <Button
          size="sm"
          variant="outline"
          className="h-8 gap-1.5 px-3 text-[12px]"
          onClick={() =>
            setEditorTarget({
              mode: "create",
              initialToml: TEMPLATE_TOML,
              originalClient: null,
            })
          }
        >
          <Plus className="size-3.5" strokeWidth={2} /> Register
        </Button>
      </header>

      {/* ── Active hero ── */}
      <ActiveHero
        active={active}
        onView={(client) => setViewingClient(client)}
        onEdit={onEditFromSource}
        onClone={onCloneFromSource}
      />

      {/* ── Gallery + variant strip ── */}
      {profiles.isLoading ? (
        <p className="mt-6 font-mono text-[12px] text-muted-foreground">
          › loading clients…
        </p>
      ) : clients.length === 0 ? (
        <EmptyClients />
      ) : (
        <>
          <header className="mt-6 flex items-center justify-between md:mt-8">
            <span className="eyebrow-strong">Library</span>
            <span className="num text-[11px] text-muted-foreground">
              {clients.length.toString().padStart(2, "0")} CLIENTS ·{" "}
              {list.length.toString().padStart(2, "0")} VARIANTS
            </span>
          </header>

          <div className="mt-2 grid grid-cols-2 gap-1.5 sm:grid-cols-3 lg:grid-cols-4">
            {clients.map((c) => (
              <ClientTile
                key={c.client}
                c={c}
                selected={selectedClient === c.client}
                onSelect={() =>
                  setSelectedClient((prev) =>
                    prev === c.client ? null : c.client,
                  )
                }
              />
            ))}
          </div>

          {selectedClient && (
            <VariantDrawer
              c={clients.find((f) => f.client === selectedClient) ?? clients[0]}
              onActivate={setPendingActivate}
              onView={(client) => setViewingClient(client)}
              onClone={onCloneFromSource}
              onEdit={onEditFromSource}
              onExtend={onExtendBundled}
              onDeleteClient={setPendingDeleteClient}
            />
          )}
        </>
      )}

      <ActivateDialog
        target={pendingActivate}
        current={active}
        onClose={() => setPendingActivate(null)}
      />
      <DeleteDialog
        target={pendingDeleteClient}
        onClose={() => setPendingDeleteClient(null)}
      />
      <ViewSourceDialog
        client={viewingClient}
        onClose={() => setViewingClient(null)}
        onClone={(source) => {
          if (viewingClient) onCloneFromSource(viewingClient, source);
          setViewingClient(null);
        }}
      />
      <EditorDialog
        target={editorTarget}
        onClose={() => setEditorTarget(null)}
      />
    </div>
  );
}

/* ───────────────────────── active hero ───────────────────────── */

function ActiveHero({
  active,
  onView,
  onEdit,
  onClone,
}: {
  active: ClientProfileSummary | null;
  onView: (client: string) => void;
  onClone: (client: string, source: string) => void;
  onEdit: (client: string, source: string) => void;
}) {
  const [pendingAction, setPendingAction] = useState<null | "clone" | "edit">(
    null,
  );
  const lazy = useClientSource(pendingAction && active ? active.client : null);
  // biome-ignore lint/correctness/useExhaustiveDependencies: callbacks are stable; rerunning when pending+data resolve is the whole point
  useEffect(() => {
    if (!pendingAction || !active || !lazy.data) return;
    if (pendingAction === "clone") onClone(active.client, lazy.data.toml);
    if (pendingAction === "edit") onEdit(active.client, lazy.data.toml);
    setPendingAction(null);
  }, [pendingAction, lazy.data]);

  if (!active) {
    return (
      <section className="overflow-hidden rounded-md border border-dashed bg-card/30 px-4 py-6">
        <div className="flex items-center gap-3">
          <AlertTriangle className="size-4 text-warn" strokeWidth={2} />
          <span className="font-mono text-[12px] text-muted-foreground">
            no active client variant · select one below to start announcing
          </span>
        </div>
      </section>
    );
  }

  return (
    <section className="relative overflow-hidden rounded-md border border-success/35 bg-card">
      <div
        className="pointer-events-none absolute inset-y-0 left-0 w-px bg-success"
        aria-hidden="true"
      />
      <div
        className="pointer-events-none absolute inset-0 opacity-[0.06]"
        style={{
          backgroundImage:
            "radial-gradient(circle at 10% 0%, var(--success) 0%, transparent 55%)",
        }}
        aria-hidden="true"
      />

      <div className="relative flex flex-col gap-4 px-4 py-4 md:flex-row md:items-center md:gap-6 md:px-5 md:py-5">
        {/* Identity */}
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2 text-success">
            <span className="dot-live" aria-hidden="true" />
            <span className="eyebrow-strong text-success">Active</span>
          </div>
          <div className="mt-1.5 flex flex-wrap items-baseline gap-x-3 gap-y-1">
            <h2
              className="num text-[20px] font-semibold leading-tight md:text-[24px]"
              title={active.client}
            >
              {active.client}
            </h2>
            <span className="num text-[14px] text-muted-foreground md:text-[15px]">
              v{active.version}
            </span>
            <EditableTag editable={active.editable} />
          </div>
          <div
            className="mt-1.5 truncate font-mono text-[10.5px] text-muted-foreground/80"
            title={active.id}
          >
            {active.id}
          </div>
        </div>

        {/* Actions */}
        <div className="flex shrink-0 flex-wrap items-center gap-2">
          <Button
            size="sm"
            variant="outline"
            onClick={() => onView(active.client)}
            className="h-8 gap-1.5 px-3 text-[12px]"
          >
            <Eye className="size-3.5" strokeWidth={2} /> Source
          </Button>
          {active.editable ? (
            <Button
              size="sm"
              variant="outline"
              onClick={() => setPendingAction("edit")}
              className="h-8 gap-1.5 px-3 text-[12px]"
              disabled={pendingAction === "edit"}
            >
              <Pencil className="size-3.5" strokeWidth={2} /> Edit
            </Button>
          ) : (
            <Button
              size="sm"
              variant="outline"
              onClick={() => setPendingAction("clone")}
              className="h-8 gap-1.5 px-3 text-[12px]"
              disabled={pendingAction === "clone"}
            >
              <Copy className="size-3.5" strokeWidth={2} /> Clone &amp; edit
            </Button>
          )}
        </div>
      </div>
    </section>
  );
}

/* ───────────────────────── client tile ───────────────────────── */

function ClientTile({
  c,
  selected,
  onSelect,
}: {
  c: Client;
  selected: boolean;
  onSelect: () => void;
}) {
  const activeVariant = c.variants.find((p) => p.active);

  return (
    <button
      type="button"
      onClick={onSelect}
      aria-pressed={selected}
      className={cn(
        "group relative overflow-hidden rounded-md border bg-card px-3 py-3 text-left transition-all",
        "hover:bg-accent/40",
        selected
          ? "border-foreground/45 ring-1 ring-foreground/20"
          : "border-border",
        activeVariant && "border-success/40",
      )}
    >
      {activeVariant && (
        <span
          className="absolute inset-y-2 left-0 w-px bg-success"
          aria-hidden="true"
        />
      )}
      <div className="flex items-center justify-between gap-2">
        <div className="num truncate text-[13px] font-semibold leading-tight">
          {c.client}
        </div>
        <ChevronRight
          className={cn(
            "size-3.5 shrink-0 text-muted-foreground/60 transition-transform",
            selected && "rotate-90 text-foreground",
          )}
          strokeWidth={2}
        />
      </div>

      <div className="mt-1.5 flex items-baseline justify-between gap-2">
        <div className="num truncate text-[11px] text-muted-foreground">
          {c.variants.length} {c.variants.length === 1 ? "variant" : "variants"}
          {c.editable && " · user"}
        </div>
        {activeVariant && (
          <span className="font-mono text-[10px] font-semibold uppercase tracking-wider text-success">
            {activeVariant.version}
          </span>
        )}
      </div>
    </button>
  );
}

/* ───────────────────────── variant drawer ───────────────────────── */

function VariantDrawer({
  c,
  onActivate,
  onView,
  onClone,
  onEdit,
  onExtend,
  onDeleteClient,
}: {
  c: Client;
  onActivate: (p: ClientProfileSummary) => void;
  onView: (client: string) => void;
  onClone: (client: string, source: string) => void;
  onEdit: (client: string, source: string) => void;
  onExtend: (client: string) => void;
  onDeleteClient: (c: Client) => void;
}) {
  const [pendingAction, setPendingAction] = useState<null | "clone" | "edit">(
    null,
  );
  const lazy = useClientSource(pendingAction ? c.client : null);
  // biome-ignore lint/correctness/useExhaustiveDependencies: callbacks are stable; rerunning when pending+data resolve is the whole point
  useEffect(() => {
    if (!pendingAction || !lazy.data) return;
    if (pendingAction === "clone") onClone(c.client, lazy.data.toml);
    if (pendingAction === "edit") onEdit(c.client, lazy.data.toml);
    setPendingAction(null);
  }, [pendingAction, lazy.data]);

  return (
    <section className="mt-1.5 overflow-hidden rounded-md border bg-card">
      <header className="flex items-center justify-between border-b px-4 py-2.5">
        <div className="flex items-center gap-2">
          <span className="eyebrow hidden sm:inline">Selected</span>
          <span className="num text-[12.5px] font-semibold">{c.client}</span>
          <EditableTag editable={c.editable} />
        </div>
        <div className="flex items-center gap-1">
          <span className="num text-[11px] text-muted-foreground">
            {c.variants.length}
            <span className="hidden sm:inline">
              {" "}
              {c.variants.length === 1 ? "variant" : "variants"}
            </span>
          </span>
          {c.hasBundled && (
            <Button
              size="sm"
              variant="ghost"
              onClick={() => onExtend(c.client)}
              title="Add a user variant on top of bundled"
              className="ml-1 h-7 gap-1 px-2 text-[11px] font-semibold text-muted-foreground hover:text-foreground"
            >
              <Plus className="size-3.5" strokeWidth={2} /> Variant
            </Button>
          )}
          <Button
            size="sm"
            variant="ghost"
            onClick={() => onView(c.client)}
            title="View doc"
            className="h-7 w-7 p-0 text-muted-foreground hover:text-foreground"
          >
            <Eye className="size-3.5" strokeWidth={2} />
          </Button>
          {c.editable ? (
            <>
              <Button
                size="sm"
                variant="ghost"
                onClick={() => setPendingAction("edit")}
                disabled={pendingAction === "edit"}
                title="Edit doc"
                className="h-7 w-7 p-0 text-muted-foreground hover:text-foreground"
              >
                <Pencil className="size-3.5" strokeWidth={2} />
              </Button>
              <Button
                size="sm"
                variant="ghost"
                onClick={() => onDeleteClient(c)}
                title="Delete client"
                className="h-7 w-7 p-0 text-muted-foreground hover:text-destructive"
              >
                <Trash2 className="size-3.5" strokeWidth={2} />
              </Button>
            </>
          ) : (
            <Button
              size="sm"
              variant="ghost"
              onClick={() => setPendingAction("clone")}
              disabled={pendingAction === "clone"}
              title="Clone doc"
              className="h-7 w-7 p-0 text-muted-foreground hover:text-foreground"
            >
              <Copy className="size-3.5" strokeWidth={2} />
            </Button>
          )}
        </div>
      </header>
      <ul className="divide-y">
        {c.variants.map((p) => (
          <VariantRow key={p.id} p={p} onActivate={onActivate} />
        ))}
      </ul>
    </section>
  );
}

function VariantRow({
  p,
  onActivate,
}: {
  p: ClientProfileSummary;
  onActivate: (p: ClientProfileSummary) => void;
}) {
  return (
    <li
      className={cn(
        "flex items-center gap-3 px-4 py-2.5 transition-colors",
        p.active ? "bg-success/[0.05]" : "hover:bg-accent/25",
      )}
    >
      {/* Status rail */}
      <span
        className={cn(
          "flex size-5 shrink-0 items-center justify-center rounded-sm",
          p.active
            ? "bg-success/20 text-success"
            : "bg-muted text-muted-foreground/40",
        )}
        aria-hidden="true"
      >
        {p.active ? (
          <Check className="size-3" strokeWidth={2.5} />
        ) : (
          <span className="size-1.5 rounded-full bg-current" />
        )}
      </span>

      {/* Version + id */}
      <div className="flex min-w-0 flex-1 items-baseline gap-3">
        <span
          className={cn(
            "num shrink-0 text-[13px] font-semibold tabular-nums",
            p.active ? "text-success" : "text-foreground",
          )}
          title={p.version}
        >
          {p.version}
        </span>
        <span
          className="fade-x num min-w-0 flex-1 truncate text-[10.5px] text-muted-foreground/75"
          title={p.id}
        >
          {p.id}
        </span>
      </div>

      {/* Activate */}
      <div className="flex shrink-0 items-center gap-1">
        {p.active ? (
          <span className="inline-flex h-7 items-center rounded-md bg-success/15 px-2.5 text-[11px] font-semibold text-success">
            Active
          </span>
        ) : (
          <Button
            size="sm"
            variant="default"
            onClick={() => onActivate(p)}
            className="h-7 px-2.5 text-[11px] font-semibold"
          >
            Activate
          </Button>
        )}
      </div>
    </li>
  );
}

function EditableTag({ editable }: { editable: boolean }) {
  return editable ? (
    <span className="hidden shrink-0 rounded-sm border border-signal/40 bg-signal/10 px-1.5 py-0.5 font-mono text-[9px] font-semibold uppercase tracking-[0.16em] text-signal sm:inline">
      User
    </span>
  ) : (
    <span className="hidden shrink-0 rounded-sm border border-border bg-muted/50 px-1.5 py-0.5 font-mono text-[9px] font-semibold uppercase tracking-[0.16em] text-muted-foreground sm:inline">
      Bundled
    </span>
  );
}

/* ───────────────────────── grouping ───────────────────────── */

interface Client {
  client: string;
  /** True if any user-owned (editable) variants are registered for this client. */
  editable: boolean;
  /** True if any bundled (read-only) variants are registered for this client. */
  hasBundled: boolean;
  variants: ClientProfileSummary[];
}

function groupByClient(list: ClientProfileSummary[]): Client[] {
  const map = new Map<string, ClientProfileSummary[]>();
  for (const p of list) {
    const arr = map.get(p.client) ?? [];
    arr.push(p);
    map.set(p.client, arr);
  }
  return [...map.entries()]
    .map(([client, variants]) => ({
      client,
      editable: variants.some((v) => v.editable),
      hasBundled: variants.some((v) => !v.editable),
      variants: [...variants].sort((a, b) =>
        compareVersionsDesc(a.version, b.version),
      ),
    }))
    .sort((a, b) => a.client.localeCompare(b.client));
}

function compareVersionsDesc(a: string, b: string): number {
  const pa = a.split(/[._-]/).map((p) => Number.parseInt(p, 10));
  const pb = b.split(/[._-]/).map((p) => Number.parseInt(p, 10));
  const len = Math.max(pa.length, pb.length);
  for (let i = 0; i < len; i++) {
    const av = Number.isFinite(pa[i]) ? pa[i] : 0;
    const bv = Number.isFinite(pb[i]) ? pb[i] : 0;
    if (av !== bv) return bv - av;
  }
  return b.localeCompare(a);
}

/* ───────────────────────── dialogs ───────────────────────── */

function ActivateDialog({
  target,
  current,
  onClose,
}: {
  target: ClientProfileSummary | null;
  current: ClientProfileSummary | null;
  onClose: () => void;
}) {
  const activate = useActivateVariant();
  const onConfirm = async () => {
    if (!target) return;
    try {
      await activate.mutateAsync(target.id);
      toast.success("Active variant updated");
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "activation failed");
    } finally {
      onClose();
    }
  };
  const fmt = (p: ClientProfileSummary | null | undefined) =>
    p ? `${p.client} ${p.version}` : "—";
  const rows: DiffListItem[] = target
    ? [{ key: "variant", from: fmt(current), to: fmt(target) }]
    : [];
  return (
    <Dialog open={!!target} onOpenChange={(v) => !v && onClose()}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <span className="eyebrow-strong">Switch · active variant</span>
          <DialogTitle className="text-base font-semibold">
            Activate variant
          </DialogTitle>
          <DialogDescription className="text-[12px]">
            The new variant takes effect on the next announce.
          </DialogDescription>
        </DialogHeader>

        <DiffList items={rows} />

        <DialogFooter>
          <Button
            type="button"
            variant="ghost"
            onClick={onClose}
            disabled={activate.isPending}
          >
            Cancel
          </Button>
          <Button
            type="button"
            onClick={onConfirm}
            disabled={activate.isPending}
          >
            {activate.isPending ? "Applying…" : "Apply changes"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function DeleteDialog({
  target,
  onClose,
}: {
  target: Client | null;
  onClose: () => void;
}) {
  const del = useDeleteClient();
  const onConfirm = async () => {
    if (!target) return;
    try {
      await del.mutateAsync(target.client);
      toast.success(`Deleted ${target.client}`);
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "delete failed");
    } finally {
      onClose();
    }
  };
  return (
    <Dialog open={!!target} onOpenChange={(v) => !v && onClose()}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <span className="eyebrow-strong text-destructive">
            Destructive · delete
          </span>
          <DialogTitle className="flex items-center gap-2 text-base font-semibold">
            <AlertTriangle
              className="size-4 text-destructive"
              strokeWidth={2}
            />
            Delete client
          </DialogTitle>
          <DialogDescription className="text-[12px]">
            Removes the client and every variant under it. This cannot be
            undone; you can re-register the doc later.
          </DialogDescription>
        </DialogHeader>

        <div className="rounded-md border bg-card/40 px-3 py-2.5 font-mono text-[11.5px]">
          <div className="flex items-baseline justify-between gap-3">
            <div>
              <div className="eyebrow text-muted-foreground">client</div>
              <div className="num mt-1 truncate text-foreground">
                {target?.client}
              </div>
            </div>
            <div className="text-right">
              <div className="eyebrow text-muted-foreground">variants</div>
              <div className="num mt-1 tabular-nums text-foreground">
                {target?.variants.length ?? 0}
              </div>
            </div>
          </div>
        </div>

        <DialogFooter>
          <Button
            type="button"
            variant="ghost"
            onClick={onClose}
            disabled={del.isPending}
          >
            Cancel
          </Button>
          <Button
            type="button"
            variant="destructive"
            onClick={onConfirm}
            disabled={del.isPending}
          >
            {del.isPending ? "Deleting…" : "Delete client"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function ViewSourceDialog({
  client,
  onClose,
  onClone,
}: {
  client: string | null;
  onClose: () => void;
  onClone: (source: string) => void;
}) {
  const q = useClientSource(client);
  return (
    <Dialog open={!!client} onOpenChange={(v) => !v && onClose()}>
      <DialogContent className="max-w-3xl">
        <DialogHeader>
          <span className="eyebrow-strong">Source · TOML</span>
          <DialogTitle className="num text-base font-semibold">
            {client}
          </DialogTitle>
          <DialogDescription className="text-[12px]">
            {q.data?.editable
              ? "User client doc — editable. Clone to duplicate."
              : "Bundled client doc — read-only. Clone to make an editable copy."}
          </DialogDescription>
        </DialogHeader>
        <pre className="max-h-[60vh] overflow-auto whitespace-pre-wrap break-all rounded-md border bg-card/40 p-3 font-mono text-[11px] leading-snug text-foreground/85">
          {q.isLoading ? "loading…" : (q.data?.toml ?? "")}
        </pre>
        <DialogFooter>
          <Button type="button" variant="ghost" onClick={onClose}>
            Close
          </Button>
          <Button
            type="button"
            onClick={() => q.data && onClone(q.data.toml)}
            disabled={!q.data}
          >
            <Copy className="mr-1.5 size-3.5" strokeWidth={2} /> Clone
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function EditorDialog({
  target,
  onClose,
}: {
  target: EditorTarget | null;
  onClose: () => void;
}) {
  const register = useRegisterClient();
  const [toml, setToml] = useState("");

  const needSource =
    !!target && target.originalClient !== null && !target.initialToml;
  const sourceQ = useClientSource(
    needSource ? (target?.originalClient ?? null) : null,
  );
  useEffect(() => {
    if (!target) return;
    if (target.initialToml) setToml(target.initialToml);
    else if (sourceQ.data) setToml(sourceQ.data.toml);
    else setToml("");
  }, [target, sourceQ.data]);

  if (!target) return null;
  const titleByMode: Record<EditorMode, string> = {
    create: "Register client",
    clone: "Clone client",
    edit: "Edit client",
    extend: "Add variant on bundled client",
  };
  const eyebrowByMode: Record<EditorMode, string> = {
    create: "Client · new",
    clone: "Client · clone",
    edit: "Client · edit",
    extend: "Client · extension",
  };
  const submitLabel: Record<EditorMode, string> = {
    create: "Register",
    clone: "Register clone",
    edit: "Save changes",
    extend: "Add variant",
  };
  const pendingLabel: Record<EditorMode, string> = {
    create: "Registering…",
    clone: "Registering…",
    edit: "Saving…",
    extend: "Adding…",
  };
  const onSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    try {
      const out = await register.mutateAsync(toml);
      toast.success(
        target.mode === "edit"
          ? `Updated ${out.client} · ${out.ids.length} variant(s)`
          : `Registered ${out.client} · ${out.ids.length} variant(s)`,
      );
      onClose();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "register failed");
    }
  };

  return (
    <Dialog open onOpenChange={(v) => !v && onClose()}>
      <DialogContent className="max-w-3xl">
        <DialogHeader>
          <span className="eyebrow-strong">{eyebrowByMode[target.mode]}</span>
          <DialogTitle className="text-base font-semibold">
            {titleByMode[target.mode]}
          </DialogTitle>
          <DialogDescription className="text-[12px]">
            {target.mode === "clone"
              ? `Cloning ${target.originalClient}. Change the \`client\` field before saving.`
              : target.mode === "edit"
                ? `Editing ${target.originalClient}. Keep the \`client\` field the same to overwrite.`
                : target.mode === "extend"
                  ? `Add a user variant overlaying bundled ${target.extendingClient}. Only \`[[variant]]\` blocks; the bundled doc supplies the base.`
                  : 'Provide a TOML client doc with `client = "…"` and one or more `[[variant]]` blocks.'}
          </DialogDescription>
        </DialogHeader>
        <form onSubmit={onSubmit} className="space-y-3">
          <Label htmlFor="toml" className="eyebrow-strong">
            TOML source
          </Label>
          <textarea
            id="toml"
            value={toml}
            onChange={(e) => setToml(e.target.value)}
            spellCheck={false}
            className="h-[60vh] w-full resize-none rounded-md border bg-card/40 p-3 font-mono text-[11.5px] leading-snug text-foreground outline-none focus:border-foreground/40"
          />
          <DialogFooter>
            <Button type="button" variant="ghost" onClick={onClose}>
              Cancel
            </Button>
            <Button type="submit" disabled={register.isPending || !toml}>
              {register.isPending
                ? pendingLabel[target.mode]
                : submitLabel[target.mode]}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function EmptyClients() {
  return (
    <div className="mt-6 rounded-md border border-dashed bg-card/30 p-10 text-center">
      <div className="text-[13px] font-medium">No clients registered</div>
      <p className="mt-1 font-mono text-[11px] text-muted-foreground">
        register a TOML client doc to start emulating
      </p>
    </div>
  );
}

function extensionTemplate(client: string): string {
  return `# Extension overlay on bundled ${client}.
# Only [[variant]] blocks; base config is inherited from the bundled doc.
client = "${client}"

[[variant]]
version = "x.y.z"
headers_patch = [
    { name = "User-Agent", value = "${client}/x.y.z" },
]

[variant.peer_id_generator.algorithm]
# Override only what changed in this version (e.g. version-encoded prefix).
# pattern = "-..-[A-Za-z0-9]{12}"
`;
}

const TEMPLATE_TOML = `client = "my-client"
display_name = "My client"
query = "info_hash={infohash}&peer_id={peerid}&port={port}&uploaded={uploaded}&downloaded={downloaded}&left={left}&event={event}&numwant={numwant}&compact=1"
numwant = 200
numwant_on_stop = 0

[[request_headers]]
name = "User-Agent"
value = "my-client/1.0.0"

[peer_id_generator]
refresh_on = "torrent_volatile"

[peer_id_generator.algorithm]
type = "regex"
pattern = "-MC1000-[A-Za-z0-9]{12}"

[url_encoder]
encoding_exclusion_pattern = "[A-Za-z0-9_~\\\\(\\\\)\\\\!\\\\.\\\\*-]"
encoded_hex_case = "lower"

[[variant]]
version = "1.0.0"

[[variant]]
version = "1.1.0"
headers_patch = [
    { name = "User-Agent", value = "my-client/1.1.0" },
]

[variant.peer_id_generator.algorithm]
pattern = "-MC1100-[A-Za-z0-9]{12}"
`;
