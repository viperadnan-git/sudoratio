// Lightweight context for the active preset filter. "all" = no filter; anything else
// is a preset id that scopes the dashboard chip strip and torrent list.

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from "react";

const STORAGE_KEY = "sudoratio.preset.selected";

interface PresetSelection {
  activeId: string;
  setActive: (id: string) => void;
}

const Ctx = createContext<PresetSelection | null>(null);

export function PresetSelectionProvider({
  children,
}: {
  children: React.ReactNode;
}) {
  const [activeId, setActiveState] = useState<string>(() => {
    if (typeof window === "undefined") return "all";
    return localStorage.getItem(STORAGE_KEY) ?? "all";
  });

  useEffect(() => {
    if (typeof window === "undefined") return;
    if (activeId === "all") {
      localStorage.removeItem(STORAGE_KEY);
    } else {
      localStorage.setItem(STORAGE_KEY, activeId);
    }
  }, [activeId]);

  const setActive = useCallback(
    (id: string) => setActiveState(id || "all"),
    [],
  );
  const value = useMemo(() => ({ activeId, setActive }), [activeId, setActive]);
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function usePresetSelection(): PresetSelection {
  const ctx = useContext(Ctx);
  if (!ctx) throw new Error("usePresetSelection: missing provider");
  return ctx;
}
