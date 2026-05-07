// Reusable atomic field components, bound to TanStack Form via context.
//
// Each atom calls `useFieldContext<T>()` to get the bound field and renders
// shadcn `<Field>` markup. Used as `<form.AppField name="…"><Atom ... /></form.AppField>`,
// or composed inside `withFieldGroup`/`withForm` render props.

import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Field,
  FieldDescription,
  FieldError,
  FieldLabel,
} from "@/components/ui/field";
import { InlineEdit } from "@/components/ui/inline-edit";
import { Input } from "@/components/ui/input";
import { useFieldContext } from "@/lib/form-contexts";
import { isHexColor, PRESET_SWATCHES } from "@/lib/preset-colors";
import { useProfiles } from "@/lib/queries";
import { cn } from "@/lib/utils";

/* ───────────────────────────── Helpers ───────────────────────────── */

function useInvalid(field: { state: { meta: { isTouched: boolean; isValid: boolean } } }) {
  return field.state.meta.isTouched && !field.state.meta.isValid;
}

/* ───────────────────────────── Number ────────────────────────────── */

export interface NumRowProps {
  label: string;
  hint?: string;
  min?: number;
  max?: number;
  step?: string;
  inputWidthCls?: string;
}

/** Numeric row: label left, right-aligned input. Empty input becomes 0. */
export function NumberRow({
  label,
  hint,
  min,
  max,
  step,
  inputWidthCls = "w-20",
}: NumRowProps) {
  const field = useFieldContext<number>();
  const isInvalid = useInvalid(field);
  const v = field.state.value;
  return (
    <Field
      orientation="horizontal"
      data-invalid={isInvalid}
      className="flex min-h-[3rem] items-center justify-between gap-3 px-3 py-2 md:px-4"
    >
      <div className="min-w-0 flex-1">
        <FieldLabel
          htmlFor={field.name}
          className="text-[12.5px] font-medium leading-tight"
        >
          {label}
        </FieldLabel>
        {hint && (
          <FieldDescription className="mt-0.5 font-mono text-[10px] uppercase tracking-wider text-muted-foreground/65">
            {hint}
          </FieldDescription>
        )}
        {isInvalid && (
          <FieldError
            errors={field.state.meta.errors}
            className="mt-0.5"
          />
        )}
      </div>
      <Input
        id={field.name}
        name={field.name}
        type="number"
        min={min}
        max={max}
        step={step}
        aria-invalid={isInvalid}
        className={cn(
          "h-7 px-2 text-right font-mono text-[12.5px] tabular-nums",
          inputWidthCls,
        )}
        value={Number.isFinite(v) ? v : ""}
        onBlur={field.handleBlur}
        onChange={(e) => {
          const raw = e.currentTarget.value;
          const n = raw === "" ? 0 : Number(raw);
          field.handleChange(Number.isFinite(n) ? n : 0);
        }}
      />
    </Field>
  );
}

/** Variant where empty input means `null` (e.g. engine config "auto" knobs). */
export function NullableNumberRow({
  label,
  hint,
  min,
  max,
  step,
  placeholder = "auto",
  inputWidthCls = "w-24",
}: NumRowProps & { placeholder?: string }) {
  const field = useFieldContext<number | null>();
  const isInvalid = useInvalid(field);
  const v = field.state.value;
  return (
    <Field
      orientation="horizontal"
      data-invalid={isInvalid}
      className="flex min-h-[3rem] items-center justify-between gap-3 px-3 py-2 md:px-4"
    >
      <div className="min-w-0 flex-1">
        <FieldLabel
          htmlFor={field.name}
          className="text-[12.5px] font-medium leading-tight"
        >
          {label}
        </FieldLabel>
        {hint && (
          <FieldDescription className="mt-0.5 font-mono text-[10px] uppercase tracking-wider text-muted-foreground/65">
            {hint}
          </FieldDescription>
        )}
        {isInvalid && (
          <FieldError
            errors={field.state.meta.errors}
            className="mt-0.5"
          />
        )}
      </div>
      <Input
        id={field.name}
        name={field.name}
        type="number"
        min={min}
        max={max}
        step={step}
        placeholder={placeholder}
        aria-invalid={isInvalid}
        className={cn(
          "h-7 px-2 text-right font-mono text-[12.5px] tabular-nums",
          inputWidthCls,
        )}
        value={v == null ? "" : v}
        onBlur={field.handleBlur}
        onChange={(e) => {
          const raw = e.currentTarget.value;
          if (raw === "") {
            field.handleChange(null);
            return;
          }
          const n = Number(raw);
          field.handleChange(Number.isFinite(n) ? n : 0);
        }}
      />
    </Field>
  );
}

/** Bare number input (no row layout) — used inside Range pairs. */
export function NumberInput({
  min,
  max,
  step,
  className,
}: {
  min?: number;
  max?: number;
  step?: string;
  className?: string;
}) {
  const field = useFieldContext<number>();
  const isInvalid = useInvalid(field);
  const v = field.state.value;
  return (
    <Input
      id={field.name}
      name={field.name}
      type="number"
      min={min}
      max={max}
      step={step}
      aria-invalid={isInvalid}
      className={cn(
        "h-7 w-20 px-2 text-right font-mono text-[12.5px] tabular-nums",
        className,
      )}
      value={Number.isFinite(v) ? v : ""}
      onBlur={field.handleBlur}
      onChange={(e) => {
        const raw = e.currentTarget.value;
        const n = raw === "" ? 0 : Number(raw);
        field.handleChange(Number.isFinite(n) ? n : 0);
      }}
    />
  );
}

/* ───────────────────────────── Boolean ───────────────────────────── */

export function CheckboxRow({
  label,
  hint,
}: {
  label: string;
  hint?: string;
}) {
  const field = useFieldContext<boolean>();
  return (
    <Field
      orientation="horizontal"
      className="flex items-center justify-between gap-3 px-3 py-2 md:px-4"
    >
      <FieldLabel
        htmlFor={field.name}
        className="min-w-0 flex-1 cursor-pointer text-[12.5px] font-medium leading-tight"
      >
        {label}
        {hint && (
          <span className="mt-0.5 block font-mono text-[10px] font-normal uppercase tracking-wider text-muted-foreground/65">
            {hint}
          </span>
        )}
      </FieldLabel>
      <Checkbox
        id={field.name}
        checked={!!field.state.value}
        onCheckedChange={(v) => field.handleChange(!!v)}
      />
    </Field>
  );
}

/* ───────────────────────────── Text + InlineEdit ──────────────────── */

export function InlineEditField({
  placeholder,
  startInEditMode,
  ariaLabel,
  className,
  disabled,
}: {
  placeholder?: string;
  startInEditMode?: boolean;
  ariaLabel?: string;
  className?: string;
  disabled?: boolean;
}) {
  const field = useFieldContext<string>();
  const isInvalid = useInvalid(field);
  // No shadcn `Field` wrapper here: its default vertical orientation forces
  // `*:w-full` on direct children, which would stretch the InlineEdit beyond
  // its content. We need it to hug the typed text instead.
  return (
    <div data-invalid={isInvalid} className="inline-flex max-w-full flex-col">
      <InlineEdit
        value={field.state.value}
        onChange={(v) => field.handleChange(v)}
        placeholder={placeholder}
        disabled={disabled}
        startInEditMode={startInEditMode}
        ariaLabel={ariaLabel}
        className={className}
      />
      {isInvalid && <FieldError errors={field.state.meta.errors} />}
    </div>
  );
}

/* ───────────────────────────── Color picker ───────────────────────── */

export function ColorPickerField({ disabled }: { disabled?: boolean }) {
  const field = useFieldContext<string>();
  const value = field.state.value;
  return (
    <Popover>
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
      <PopoverContent align="start" sideOffset={8} className="w-56 gap-2 p-2">
        <div className="grid grid-cols-4 gap-1.5">
          {PRESET_SWATCHES.map((sw) => {
            const active = value.toLowerCase() === sw.hex;
            return (
              <button
                key={sw.hex}
                type="button"
                aria-label={sw.label}
                data-active={active}
                onClick={() => field.handleChange(sw.hex)}
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
          <NativeColorTile
            value={value}
            onChange={(v) => field.handleChange(v)}
          />
        </div>
        <div className="border-t pt-2">
          <FieldLabel
            htmlFor="preset-color-hex"
            className="mb-1 block font-mono text-[10px] uppercase tracking-wider text-muted-foreground/70"
          >
            Custom hex
          </FieldLabel>
          <Input
            id="preset-color-hex"
            value={value}
            onChange={(e) => field.handleChange(e.currentTarget.value)}
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

function NativeColorTile({
  value,
  onChange,
}: {
  value: string;
  onChange: (hex: string) => void;
}) {
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
        <span className="text-[11px] font-bold text-white drop-shadow">✓</span>
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

/* ───────────────────────── Client profile picker ──────────────────── */

const SELECT_CLS = cn(
  "h-7 rounded-md border border-input bg-transparent px-2 text-[12px] outline-none transition-colors",
  "focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50",
  "disabled:cursor-not-allowed disabled:opacity-50",
);

export function ClientProfileField() {
  const field = useFieldContext<string | null>();
  const value = field.state.value;
  const { data: profiles } = useProfiles();
  const list = profiles ?? [];

  const byClient = new Map<string, typeof list>();
  for (const p of list) {
    const arr = byClient.get(p.client) ?? [];
    arr.push(p);
    byClient.set(p.client, arr);
  }
  for (const [, arr] of byClient) {
    arr.sort((a, b) =>
      (b.version || b.id).localeCompare(a.version || a.id, undefined, {
        numeric: true,
        sensitivity: "base",
      }),
    );
  }
  const clientNames = Array.from(byClient.keys()).sort((a, b) =>
    a.localeCompare(b),
  );

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
      field.handleChange(null);
      return;
    }
    const first = byClient.get(client)?.[0];
    if (first) field.handleChange(first.id);
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
          onChange={(e) => field.handleChange(e.currentTarget.value)}
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
