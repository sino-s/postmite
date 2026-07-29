/* eslint-disable react-refresh/only-export-components */
import { createContext, useContext, useEffect, useMemo, useState, type ReactNode } from "react";

export const themes = ["light", "dark", "system"] as const;
export const densities = ["comfortable", "compact"] as const;
export type Theme = (typeof themes)[number];
export type Density = (typeof densities)[number];

type Preferences = {
  theme: Theme;
  density: Density;
  setTheme: (theme: Theme) => void;
  setDensity: (density: Density) => void;
};

const PreferencesContext = createContext<Preferences | null>(null);
const themeStorageKey = "postmite.theme";
const densityStorageKey = "postmite.density";

function storedValue<T extends readonly string[]>(key: string, values: T, fallback: T[number]) {
  const value = typeof window === "undefined" ? null : window.localStorage.getItem(key);
  return values.includes(value ?? "") ? (value as T[number]) : fallback;
}

export function PreferencesProvider({ children }: { children: ReactNode }) {
  const [theme, setTheme] = useState<Theme>(() => storedValue(themeStorageKey, themes, "system"));
  const [density, setDensity] = useState<Density>(() => storedValue(densityStorageKey, densities, "comfortable"));

  useEffect(() => {
    const root = document.documentElement;
    const media = typeof window.matchMedia === "function"
      ? window.matchMedia("(prefers-color-scheme: dark)")
      : { matches: false, addEventListener: () => undefined, removeEventListener: () => undefined };
    const setDocumentTheme = () => {
      root.dataset.theme = theme;
      root.dataset.resolvedTheme = theme === "system" ? (media.matches ? "dark" : "light") : theme;
    };
    setDocumentTheme();
    media.addEventListener("change", setDocumentTheme);
    root.dataset.density = density;
    window.localStorage.setItem(themeStorageKey, theme);
    window.localStorage.setItem(densityStorageKey, density);
    return () => media.removeEventListener("change", setDocumentTheme);
  }, [density, theme]);

  const value = useMemo(() => ({ theme, density, setTheme, setDensity }), [density, theme]);
  return <PreferencesContext.Provider value={value}>{children}</PreferencesContext.Provider>;
}

export function usePreferences() {
  const preferences = useContext(PreferencesContext);
  if (!preferences) {
    throw new Error("usePreferences must be used inside PreferencesProvider");
  }
  return preferences;
}
