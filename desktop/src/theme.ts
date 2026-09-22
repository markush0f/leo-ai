/** Theme values supported by CSS and persisted in browser storage. */
export type Theme = "light" | "dark";

const KEY = "ira-theme";

/** Uses a saved preference first, then the operating system's color scheme. */
export function readTheme(): Theme {
  const stored = localStorage.getItem(KEY);
  if (stored === "light" || stored === "dark") return stored;
  return window.matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark";
}

/** Updates the CSS selector on the root element and persists the choice. */
export function applyTheme(theme: Theme) {
  document.documentElement.dataset.theme = theme;
  localStorage.setItem(KEY, theme);
}
