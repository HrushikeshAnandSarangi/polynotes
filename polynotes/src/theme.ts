import { createSignal } from "solid-js";

export type Theme = "cream" | "dark" | "mono";

const THEME_KEY = "polynotes_theme";

function loadTheme(): Theme {
  const saved = localStorage.getItem(THEME_KEY);
  if (saved === "dark" || saved === "mono" || saved === "cream") return saved;
  return "cream";
}

const [theme, setThemeSignal] = createSignal<Theme>(loadTheme());

export function getTheme() {
  return theme;
}

export function setTheme(t: Theme) {
  localStorage.setItem(THEME_KEY, t);
  setThemeSignal(t);
}
