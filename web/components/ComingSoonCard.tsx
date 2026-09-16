export function ComingSoonCard({ name }: { name: string }) {
  return (
    <div
      className="flex flex-col gap-3 rounded-2xl border p-6 opacity-60"
      style={{ background: "var(--bg-card)", borderColor: "var(--border-soft)" }}
    >
      <div className="flex items-center justify-between">
        <span className="text-base font-semibold" style={{ color: "var(--text)" }}>
          {name}
        </span>
        <span
          className="rounded-full px-2.5 py-1 text-[11px] font-semibold"
          style={{ background: "var(--bg-surface)", color: "var(--text-muted)" }}
        >
          Coming soon
        </span>
      </div>
      <p className="text-sm" style={{ color: "var(--text-muted)" }}>
        Not built by CI yet. Check back after platform support lands.
      </p>
    </div>
  );
}
