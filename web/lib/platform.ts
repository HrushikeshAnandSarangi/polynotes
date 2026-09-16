import type { ReleaseAsset } from "./github";

export type PlatformId = "windows" | "linux" | "macos" | "android";

export interface PlatformAsset {
  label: string;
  url: string;
  sizeMb: number;
}

export interface PlatformGroup {
  id: PlatformId;
  name: string;
  available: boolean;
  assets: PlatformAsset[];
}

function matchAssets(assets: ReleaseAsset[], predicate: (name: string) => boolean): PlatformAsset[] {
  return assets
    .filter((a) => predicate(a.name.toLowerCase()))
    .map((a) => ({
      label: labelFor(a.name),
      url: a.browser_download_url,
      sizeMb: Math.round((a.size / (1024 * 1024)) * 10) / 10,
    }));
}

function labelFor(name: string): string {
  const lower = name.toLowerCase();
  if (lower.endsWith(".msi")) return "MSI Installer";
  if (lower.endsWith(".exe")) return "EXE Installer";
  if (lower.endsWith(".deb")) return "Debian Package (.deb)";
  if (lower.endsWith(".appimage")) return "AppImage";
  return name;
}

/**
 * Groups raw release assets by platform. Windows/Linux are derived live
 * from whatever the API actually returned. macOS and Android are always
 * `available: false` regardless of asset contents — CI does not build
 * these today, and the gallery must never imply otherwise.
 */
export function groupAssetsByPlatform(assets: ReleaseAsset[]): PlatformGroup[] {
  const windows = matchAssets(assets, (n) => n.endsWith(".msi") || n.endsWith(".exe"));
  const linux = matchAssets(assets, (n) => n.endsWith(".deb") || n.endsWith(".appimage"));

  return [
    { id: "windows", name: "Windows", available: windows.length > 0, assets: windows },
    { id: "linux", name: "Linux", available: linux.length > 0, assets: linux },
    { id: "macos", name: "macOS", available: false, assets: [] },
    { id: "android", name: "Android", available: false, assets: [] },
  ];
}

/** Best-effort client-side OS detection for highlighting the relevant card. */
export function detectPlatform(): PlatformId | null {
  if (typeof navigator === "undefined") return null;
  const ua = navigator.userAgent.toLowerCase();
  const platform = (navigator.platform ?? "").toLowerCase();

  if (ua.includes("android")) return "android";
  if (ua.includes("win") || platform.includes("win")) return "windows";
  if (ua.includes("mac") || platform.includes("mac")) return "macos";
  if (ua.includes("linux") || platform.includes("linux")) return "linux";
  return null;
}
