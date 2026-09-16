import type { PlatformGroup } from "../lib/platform";

export function ReleaseCard({
  group,
  recommended,
}: {
  group: PlatformGroup;
  recommended: boolean;
}) {
  return (
    <div
      className="flex flex-col gap-4 rounded-2xl border p-6 transition-shadow"
      style={{
        background: "var(--bg-card)",
        borderColor: recommended ? "var(--accent)" : "var(--border-soft)",
        borderWidth: recommended ? 2 : 1,
      }}
    >
      <div className="flex items-center justify-between">
        <span className="text-base font-semibold" style={{ color: "var(--text)" }}>
          {group.name}
        </span>
        {recommended && (
          <span
            className="rounded-full px-2.5 py-1 text-[11px] font-semibold"
            style={{ background: "var(--accent)", color: "var(--accent-fg)" }}
          >
            Recommended for your system
          </span>
        )}
      </div>

      <div className="flex flex-col gap-2">
        {group.assets.map((asset) => (
          <a
            key={asset.url}
            href={asset.url}
            className="flex items-center justify-between rounded-xl px-4 py-3 text-sm font-semibold transition-transform active:scale-[0.98]"
            style={{ background: "var(--accent)", color: "var(--accent-fg)" }}
          >
            <span>{asset.label}</span>
            <span className="text-xs font-normal opacity-90">{asset.sizeMb} MB</span>
          </a>
        ))}
      </div>
    </div>
  );
}
