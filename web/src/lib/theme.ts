// Theme handling. Default is "dark"; persists choice in localStorage and toggles the
// `dark` class on <html>. The [`THEME_INIT_SCRIPT`] runs synchronously in <head> to avoid a
// flash-of-wrong-theme on first paint.

export type Theme = "dark" | "light";

export const THEME_STORAGE_KEY = "sudoratio.theme";

/**
 * Inline script injected into the document head via TanStack Router's `head.scripts`.
 * Reads the persisted theme (defaulting to "dark") and applies the `dark` class before React
 * hydration runs.
 */
export const THEME_INIT_SCRIPT = `(function(){try{var t=localStorage.getItem(${JSON.stringify(THEME_STORAGE_KEY)});if(t!=="light"){document.documentElement.classList.add("dark")}else{document.documentElement.classList.remove("dark")}}catch(_){document.documentElement.classList.add("dark")}})();`;

export function getStoredTheme(): Theme {
  if (typeof localStorage === "undefined") return "dark";
  try {
    const v = localStorage.getItem(THEME_STORAGE_KEY);
    return v === "light" ? "light" : "dark";
  } catch {
    return "dark";
  }
}

export function setStoredTheme(theme: Theme): void {
  try {
    localStorage.setItem(THEME_STORAGE_KEY, theme);
  } catch {
    /* noop */
  }
}

export function applyTheme(theme: Theme): void {
  if (typeof document === "undefined") return;
  const root = document.documentElement;
  if (theme === "dark") root.classList.add("dark");
  else root.classList.remove("dark");
}
