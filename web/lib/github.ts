const REPO = "HrushikeshAnandSarangi/polynotes";

export interface ReleaseAsset {
  name: string;
  browser_download_url: string;
  size: number;
}

export interface Release {
  tag_name: string;
  html_url: string;
  published_at: string;
  assets: ReleaseAsset[];
}

/**
 * Fetches the latest published GitHub Release server-side. Returns `null` on
 * any failure (rate limit, offline, no releases yet) so callers can fall
 * back to a plain link to the repo's releases page instead of crashing.
 */
export async function fetchLatestRelease(): Promise<Release | null> {
  try {
    const res = await fetch(`https://api.github.com/repos/${REPO}/releases/latest`, {
      headers: { Accept: "application/vnd.github+json" },
      next: { revalidate: 3600 },
    });
    if (!res.ok) return null;
    return (await res.json()) as Release;
  } catch {
    return null;
  }
}

export const RELEASES_URL = `https://github.com/${REPO}/releases/latest`;
export const REPO_URL = `https://github.com/${REPO}`;
