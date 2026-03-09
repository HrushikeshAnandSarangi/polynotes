import { createSignal, onMount } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import type { Theme } from "../theme";
import { getTheme, setTheme } from "../theme";

interface Props {
  onBack: () => void;
  onClearAll: () => void;
}

const THEMES: { id: Theme; label: string; desc: string; preview: string[] }[] = [
  {
    id: "cream",
    label: "Cream",
    desc: "Warm & rich contrast",
    preview: ["#FDFBF7", "#D17A43", "#4A3B32"],
  },
  {
    id: "dark",
    label: "Dark",
    desc: "Deep & focused",
    preview: ["#0F0F11", "#6366F1", "#FAFAFA"],
  },
  {
    id: "mono",
    label: "Mono",
    desc: "Clean & minimal",
    preview: ["#FAFAFA", "#171717", "#0A0A0A"],
  },
];

const MODEL_PATH_KEY = "polynotes_model_path";

export function SettingsPage(props: Props) {
  let confirmRef!: HTMLDialogElement;

  const [modelPath, setModelPath] = createSignal<string>(
    localStorage.getItem(MODEL_PATH_KEY) ?? ""
  );
  
  const [isDownloading, setIsDownloading] = createSignal(false);
  const [downloadProgress, setDownloadProgress] = createSignal(0);

  // On mount: push any locally-persisted path into the backend
  onMount(async () => {
    const saved = localStorage.getItem(MODEL_PATH_KEY) ?? "";
    if (saved) {
      await invoke("set_model_path", { path: saved });
    }
  });

  async function browseForModel() {
    const selected = await openDialog({
      title: "Select Whisper Model",
      filters: [{ name: "GGML Model", extensions: ["bin"] }],
      multiple: false,
      directory: false,
    });

    if (typeof selected === "string" && selected.length > 0) {
      setModelPath(selected);
      localStorage.setItem(MODEL_PATH_KEY, selected);
      await invoke("set_model_path", { path: selected });
    }
  }

  const displayPath = () => {
    const p = modelPath();
    if (!p) return "";
    // Show only the filename for readability
    return p.replace(/\\/g, "/").split("/").pop() ?? p;
  };

  return (
    <div class="min-h-screen flex flex-col" style={{ background: "var(--bg)" }}>

      {/* ── Header ── */}
      <header
        class="sticky top-0 z-10 flex items-center gap-3 px-4 sm:px-6 pt-10 pb-3 max-w-2xl mx-auto w-full"
        style={{ background: "var(--bg)" }}
      >
        <button
          onClick={props.onBack}
          class="w-9 h-9 rounded-xl flex items-center justify-center flex-shrink-0"
          style={{ background: "var(--bg-surface2)", color: "var(--text-muted)" }}
          title="Back"
        >
          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.2" d="M15 19l-7-7 7-7" />
          </svg>
        </button>
        <span class="text-sm font-semibold" style={{ color: "var(--text)" }}>Settings</span>
      </header>

      <div class="mx-4 sm:mx-6 h-px max-w-2xl mx-auto w-full" style={{ background: "var(--border-soft)" }} />

      {/* ── Content ── */}
      <main class="flex-1 px-4 sm:px-6 py-8 max-w-2xl mx-auto w-full">

        {/* ── Theme picker ── */}
        <section class="mb-10 w-full">
          <p class="text-[11px] font-semibold uppercase tracking-widest mb-4" style={{ color: "var(--text-subtle)" }}>
            Appearance
          </p>
          <div class="grid grid-cols-3 gap-3">
            {THEMES.map((t) => {
              const isActive = () => getTheme()() === t.id;
              return (
                <button
                  onClick={() => setTheme(t.id)}
                  class="relative flex flex-col items-start gap-2.5 p-3.5 rounded-2xl border-2 transition-transform active:scale-95 text-left"
                  style={{
                    background: "var(--bg-card)",
                    "border-color": isActive() ? "var(--accent)" : "var(--border-soft)",
                  }}
                >
                  {/* Color swatch */}
                  <div class="w-full h-12 rounded-xl overflow-hidden flex shadow-sm">
                    {t.preview.map((c) => (
                      <div class="flex-1" style={{ background: c }} />
                    ))}
                  </div>
                  {/* Label */}
                  <div class="flex flex-col gap-0.5 mt-1">
                    <span class="text-sm font-semibold leading-tight" style={{ color: "var(--text)" }}>
                      {t.label}
                    </span>
                    <span class="text-[11px] leading-tight" style={{ color: "var(--text-muted)" }}>
                      {t.desc}
                    </span>
                  </div>
                  {/* Active tick */}
                  {isActive() && (
                    <div
                      class="absolute top-2 right-2 w-4 h-4 rounded-full flex items-center justify-center"
                      style={{ background: "var(--accent)" }}
                    >
                      <svg class="w-2.5 h-2.5" fill="none" stroke="var(--accent-fg)" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="3" d="M5 13l4 4L19 7" />
                      </svg>
                    </div>
                  )}
                </button>
              );
            })}
          </div>
        </section>

        {/* ── Whisper Model ── */}
        <section class="mb-10 w-full">
          <p class="text-[11px] font-semibold uppercase tracking-widest mb-3" style={{ color: "var(--text-subtle)" }}>
            Transcription
          </p>
          <div
            class="rounded-2xl overflow-hidden border flex flex-col"
            style={{ background: "var(--bg-card)", "border-color": "var(--border-soft)" }}
          >
            {/* Built-in Model Option */}
            <button
              onClick={async () => {
                setModelPath(""); // empty triggers compile-time default in lib.rs
                localStorage.setItem(MODEL_PATH_KEY, "");
                await invoke("set_model_path", { path: "" });
              }}
              class="flex flex-col text-left px-4 py-3.5 transition-colors border-b hover:bg-[var(--bg-surface)] relative"
              style={{ "border-color": "var(--border-soft)" }}
            >
              <div class="flex items-center gap-2">
                <span class="text-sm font-semibold" style={{ color: "var(--text)" }}>Built-in Model (Fast)</span>
                {(!modelPath() || modelPath() === "") && (
                  <span class="text-[10px] font-semibold px-1.5 py-0.5 rounded-full bg-[var(--accent)] text-[var(--accent-fg)]">Active</span>
                )}
              </div>
              <span class="text-[12px] mt-1" style={{ color: "var(--text-muted)" }}>Bundle default. Highly quantized for speed, but lower accuracy.</span>
            </button>

            {/* Download Recommended Option */}
            <div class="flex flex-col transition-colors border-b relative" style={{ "border-color": "var(--border-soft)" }}>
              <div class="flex items-center justify-between px-4 py-3.5 gap-3">
                <div class="flex flex-col text-left flex-1">
                  <div class="flex items-center gap-2">
                    <span class="text-sm font-semibold" style={{ color: "var(--text)" }}>Recommended Base Model</span>
                    {modelPath().includes("ggml-base.bin") && (
                      <span class="text-[10px] font-semibold px-1.5 py-0.5 rounded-full bg-[var(--accent)] text-[var(--accent-fg)]">Active</span>
                    )}
                  </div>
                  <span class="text-[12px] mt-1 pr-2" style={{ color: "var(--text-muted)" }}>~141MB download. Highest accuracy, Hinglish & Multi-language support.</span>
                </div>
                <button
                  onClick={async () => {
                    if (isDownloading()) return;
                    setIsDownloading(true);
                    setDownloadProgress(0);
                    
                    const unsubscribe = await listen<{ downloaded: number; total: number }>("download_progress", (event) => {
                      const p = Math.round((event.payload.downloaded / event.payload.total) * 100);
                      setDownloadProgress(p);
                    });

                    try {
                      const path = await invoke<string>("download_model");
                      setModelPath(path);
                      localStorage.setItem(MODEL_PATH_KEY, path);
                      // Model path is automatically set in backend by the command
                    } catch (e) {
                      alert(`Failed to download model: ${e}`);
                    } finally {
                      setIsDownloading(false);
                      setDownloadProgress(0);
                      unsubscribe();
                    }
                  }}
                  disabled={isDownloading()}
                  class="flex-shrink-0 px-3 py-1.5 rounded-xl text-xs font-semibold transition-all disabled:opacity-50 flex items-center gap-1.5"
                  style={{ background: "var(--bg-surface2)", color: "var(--text)" }}
                >
                  {isDownloading() ? (
                    <>
                      <svg class="w-3.5 h-3.5 animate-spin" fill="none" viewBox="0 0 24 24">
                        <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                        <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                      </svg>
                      {downloadProgress()}%
                    </>
                  ) : (
                    <>
                      <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" />
                      </svg>
                      Download
                    </>
                  )}
                </button>
              </div>
              
              {/* Progress Bar Fill */}
              {isDownloading() && (
                <div class="absolute bottom-0 left-0 h-0.5 bg-[var(--accent)] transition-all duration-200" style={{ width: `${downloadProgress()}%` }} />
              )}
            </div>

            {/* Custom File Browser Option */}
            <div class="flex items-center justify-between px-4 py-3.5 gap-3">
              <div class="flex flex-col gap-0.5 min-w-0">
                <div class="flex items-center gap-2">
                  <span class="text-sm font-semibold" style={{ color: "var(--text)" }}>Custom Local Model</span>
                  {(modelPath() && !modelPath().includes("ggml-base.bin")) && (
                    <span class="text-[10px] font-semibold px-1.5 py-0.5 rounded-full bg-[var(--accent)] text-[var(--accent-fg)]">Active</span>
                  )}
                </div>
                <span
                  class="text-[12px] truncate"
                  style={{ color: "var(--text-muted)" }}
                  title={modelPath() || "No model selected"}
                >
                  {displayPath() || "Select any local .bin file"}
                </span>
              </div>
              <button
                onClick={browseForModel}
                disabled={isDownloading()}
                class="flex-shrink-0 px-3 py-1.5 rounded-xl text-xs font-semibold transition-colors disabled:opacity-50"
                style={{ background: "var(--bg-surface2)", color: "var(--text)" }}
              >
                Browse…
              </button>
            </div>
          </div>
        </section>

        {/* ── Data ── */}
        <section>
          <p class="text-[11px] font-semibold uppercase tracking-widest mb-3" style={{ color: "var(--text-subtle)" }}>
            Data
          </p>
          <div
            class="rounded-2xl overflow-hidden border"
            style={{ background: "var(--bg-card)", "border-color": "var(--border-soft)" }}
          >
            <button
              onClick={() => confirmRef.showModal()}
              class="w-full flex items-center justify-between px-4 py-3.5 text-left transition-colors"
              style={{ color: "var(--red)" }}
            >
              <span class="text-sm font-medium">Delete All Notes</span>
              <svg class="w-4 h-4 opacity-70" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                  d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
              </svg>
            </button>
          </div>
        </section>
      </main>

      {/* ── Confirm dialog ── */}
      <dialog
        ref={confirmRef!}
        class="rounded-2xl border p-6 max-w-xs w-full"
        style={{ background: "var(--bg)", "border-color": "var(--border)", color: "var(--text)" }}
      >
        <h3 class="text-base font-semibold mb-1" style={{ color: "var(--text)" }}>
          Delete all notes?
        </h3>
        <p class="text-sm mb-5" style={{ color: "var(--text-muted)" }}>
          This is permanent and cannot be undone.
        </p>
        <div class="flex gap-2">
          <button
            onClick={() => confirmRef.close()}
            class="flex-1 py-2.5 rounded-xl border text-sm font-medium transition-colors"
            style={{ "border-color": "var(--border)", color: "var(--text-muted)" }}
          >
            Cancel
          </button>
          <button
            onClick={() => { props.onClearAll(); confirmRef.close(); }}
            class="flex-1 py-2.5 rounded-xl text-sm font-medium text-white transition-colors"
            style={{ background: "var(--red)" }}
          >
            Delete All
          </button>
        </div>
      </dialog>
    </div>
  );
}
