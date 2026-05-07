// Click-to-edit text field. Renders as plain text until activated, then swaps to a real
// `<input>` for editing. Commits on Enter/blur, cancels on Escape. Tab/Enter/F2 activate.
//
// Pattern: toggle-based (separate display + edit nodes) — preferred over contenteditable
// for accessibility (real input keyboard semantics, IME support, no Range/Selection
// quirks). Refs:
//   https://blog.logrocket.com/build-inline-editable-ui-react/
//   https://www.emgoto.com/react-inline-edit/

import { Pencil } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";

export interface InlineEditProps {
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
  disabled?: boolean;
  /** Auto-enter editing on mount (e.g. for a freshly-created draft). */
  startInEditMode?: boolean;
  /** Validate before commit; falsy return = reject and revert. */
  validate?: (v: string) => boolean;
  /** Wrapper class — applies to both display and edit states. */
  className?: string;
  /** Override styling on the input (edit state). */
  inputClassName?: string;
  /** Override styling on the display button (idle state). */
  textClassName?: string;
  ariaLabel?: string;
  /** Show the pencil affordance icon on hover. Default true. */
  showAffordance?: boolean;
}

export function InlineEdit({
  value,
  onChange,
  placeholder = "Click to edit",
  disabled,
  startInEditMode = false,
  validate,
  className,
  inputClassName,
  textClassName,
  ariaLabel = "Edit",
  showAffordance = true,
}: InlineEditProps) {
  const [editing, setEditing] = useState(startInEditMode);
  const [local, setLocal] = useState(value);
  const inputRef = useRef<HTMLInputElement | null>(null);

  useEffect(() => {
    if (!editing) setLocal(value);
  }, [value, editing]);

  useEffect(() => {
    if (editing && inputRef.current) {
      inputRef.current.focus();
      inputRef.current.select();
    }
  }, [editing]);

  const commit = () => {
    if (validate && !validate(local)) {
      setLocal(value);
      setEditing(false);
      return;
    }
    if (local !== value) onChange(local);
    setEditing(false);
  };
  const cancel = () => {
    setLocal(value);
    setEditing(false);
  };

  if (editing) {
    // `field-sizing: content` makes the input auto-grow to fit its content (modern
    // browsers); `size` is the legacy fallback. `min-w-[8ch]` enforces the minimum.
    const charCount = Math.max(local.length, placeholder.length, 1);
    return (
      <Input
        ref={inputRef}
        type="text"
        value={local}
        size={charCount}
        onChange={(e) => setLocal(e.currentTarget.value)}
        onBlur={commit}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            commit();
          } else if (e.key === "Escape") {
            e.preventDefault();
            cancel();
          }
        }}
        placeholder={placeholder}
        disabled={disabled}
        spellCheck={false}
        aria-label={ariaLabel}
        className={cn(
          "h-8 w-auto min-w-[8ch] max-w-full px-2 [field-sizing:content]",
          className,
          inputClassName,
        )}
      />
    );
  }

  const display = value || placeholder;
  const empty = !value;

  return (
    <button
      type="button"
      onClick={() => !disabled && setEditing(true)}
      onKeyDown={(e) => {
        if (disabled) return;
        if (e.key === "Enter" || e.key === " " || e.key === "F2") {
          e.preventDefault();
          setEditing(true);
        }
      }}
      aria-label={ariaLabel}
      disabled={disabled}
      className={cn(
        "group -mx-2 inline-flex h-8 max-w-full cursor-text items-center gap-1.5 rounded-md px-2 text-left transition-colors",
        "hover:bg-foreground/[0.04] focus-visible:bg-foreground/[0.05] focus-visible:outline-1 focus-visible:outline-foreground/25",
        empty && "text-muted-foreground/45",
        disabled && "cursor-not-allowed opacity-60",
        className,
        textClassName,
      )}
    >
      <span className="whitespace-nowrap">{display}</span>
      {showAffordance && !disabled && (
        <Pencil
          aria-hidden="true"
          strokeWidth={1.75}
          className="size-3 shrink-0 text-muted-foreground/55 opacity-0 transition-opacity group-hover:opacity-100 group-focus-visible:opacity-100"
        />
      )}
    </button>
  );
}
