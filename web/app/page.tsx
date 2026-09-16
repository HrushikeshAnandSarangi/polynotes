import { fetchLatestRelease, RELEASES_URL, REPO_URL } from "../lib/github";
import { groupAssetsByPlatform } from "../lib/platform";
import { PlatformGrid } from "../components/PlatformGrid";

export default async function Home() {
  const release = await fetchLatestRelease();
  const groups = release ? groupAssetsByPlatform(release.assets) : null;

  return (
    <main className="mx-auto flex min-h-screen max-w-3xl flex-col gap-10 px-6 py-16 sm:py-24">
      <header className="flex flex-col gap-4 text-center">
        <h1 className="text-3xl font-semibold sm:text-4xl" style={{ color: "var(--text)" }}>
          Polynotes
        </h1>
        <p className="mx-auto max-w-xl text-base" style={{ color: "var(--text-muted)" }}>
          Real-time multilingual lecture transcription that runs entirely on your device.
          Download the build for your system below.
        </p>
        {release && (
          <p className="text-xs" style={{ color: "var(--text-subtle)" }}>
            Latest release: {release.tag_name}
          </p>
        )}
      </header>

      {groups ? (
        <PlatformGrid groups={groups} />
      ) : (
        <div
          className="flex flex-col items-center gap-3 rounded-2xl border p-8 text-center"
          style={{ background: "var(--bg-card)", borderColor: "var(--border-soft)" }}
        >
          <p className="text-sm" style={{ color: "var(--text-muted)" }}>
            Couldn&apos;t reach GitHub to list release downloads right now.
          </p>
          <a
            href={RELEASES_URL}
            className="rounded-xl px-4 py-2 text-sm font-semibold"
            style={{ background: "var(--accent)", color: "var(--accent-fg)" }}
          >
            View releases on GitHub
          </a>
        </div>
      )}

      <footer className="mt-auto pt-10 text-center text-xs" style={{ color: "var(--text-subtle)" }}>
        <a href={REPO_URL} style={{ color: "var(--text-subtle)" }}>
          github.com/HrushikeshAnandSarangi/polynotes
        </a>
      </footer>
    </main>
  );
}
