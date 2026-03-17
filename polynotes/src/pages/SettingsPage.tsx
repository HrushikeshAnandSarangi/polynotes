import { createSignal, onMount, onCleanup, For, Show } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import type { Theme } from "../theme";
import { getTheme, setTheme } from "../theme";

interface Props {
  onBack: () => void;
  onClearAll: () => void;
}

interface ModelInfo {
  id: string;
  name: string;
  size_mb: number;
  url: string;
  quantization: string;
  description: string;
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
const SELECTED_MODEL_KEY = "polynotes_selected_model";

export function SettingsPage(props: Props) {
  let confirmRef!: HTMLDialogElement;
  let modelDropdownRef!: HTMLDivElement;

  const [modelPath, setModelPath] = createSignal<string>(
    localStorage.getItem(MODEL_PATH_KEY) ?? ""
  );
  
  const [isDownloading, setIsDownloading] = createSignal(false);
  const [downloadProgress, setDownloadProgress] = createSignal(0);
  
  // Model selection state
  const [availableModels, setAvailableModels] = createSignal<ModelInfo[]>([]);
  const [selectedModelId, setSelectedModelId] = createSignal<string>(
    localStorage.getItem(SELECTED_MODEL_KEY) ?? "base-q5_1"
  );
  const [showModelDropdown, setShowModelDropdown] = createSignal(false);

  // Close dropdown when clicking outside
  const handleClickOutside = (e: MouseEvent) => {
    if (modelDropdownRef && !modelDropdownRef.contains(e.target as Node)) {
      setShowModelDropdown(false);
    }
  };

  // On mount: load available models and set model path
  onMount(async () => {
    // Add click outside listener
    document.addEventListener("click", handleClickOutside);
    
    onCleanup(() => {
      document.removeEventListener("click", handleClickOutside);
    });
    
    // Load available models from backend
    try {
      const models = await invoke<ModelInfo[]>("get_available_models");
      setAvailableModels(models);
    } catch (e) {
      console.error("Failed to load models:", e);
    }
    
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

  async function downloadSelectedModel() {
    const modelId = selectedModelId();
    if (isDownloading() || !modelId) return;
    
    setIsDownloading(true);
    setDownloadProgress(0);
    
    const unsubscribe = await listen<{ downloaded: number; total: number }>("download_progress", (event) => {
      const p = Math.round((event.payload.downloaded / event.payload.total) * 100);
      setDownloadProgress(p);
    });

    try {
      const path = await invoke<string>("download_model", { modelId: modelId });
      setModelPath(path);
      localStorage.setItem(MODEL_PATH_KEY, path);
    } catch (e) {
      alert(`Failed to download model: ${e}`);
    } finally {
      setIsDownloading(false);
      setDownloadProgress(0);
      unsubscribe();
    }
  }

  function selectModel(modelId: string) {
    setSelectedModelId(modelId);
    localStorage.setItem(SELECTED_MODEL_KEY, modelId);
    setShowModelDropdown(false);
  }

  const isCurrentModel = (modelId: string) => {
    const path = modelPath();
    const normalizedId = modelId.replace("-q5_1", "").replace("-q5_0", "");
    return path.includes(normalizedId);
  };

  const getCurrentModelName = () => {
    const path = modelPath();
    if (!path) return "Built-in Model";
    const models = availableModels();
    for (const m of models) {
      if (path.includes(m.id) || path.includes(m.url.replace("ggml-", "").replace(".bin", ""))) {
        return m.name;
      }
    }
    return path.split(/[/\\]/).pop() ?? "Custom";
  };

  const selectedModel = () => {
    return availableModels().find(m => m.id === selectedModelId());
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
                setModelPath("");
                localStorage.setItem(MODEL_PATH_KEY, "");
                await invoke("set_model_path", { path: "" });
              }}
              class="flex flex-col text-left px-4 py-3.5 transition-colors border-b hover:bg-[var(--bg-surface)]"
              style={{ "border-color": "var(--border-soft)" }}
            >
              <div class="flex items-center gap-2">
                <span class="text-sm font-semibold" style={{ color: "var(--text)" }}>Built-in Model (Base English)</span>
                {(!modelPath() || modelPath() === "") && (
                  <span class="text-[10px] font-semibold px-1.5 py-0.5 rounded-full bg-[var(--accent)] text-[var(--accent-fg)]">Active</span>
                )}
              </div>
              <span class="text-[12px] mt-1" style={{ color: "var(--text-muted)" }}>76 MB • English only. Optimized for speed. Bundled.</span>
            </button>

            {/* Download Model Section */}
            <div class="border-b" style={{ "border-color": "var(--border-soft)" }}>
              <div class="flex items-center justify-between px-4 py-3.5 gap-3">
                <div class="flex flex-col text-left flex-1">
                  <div class="flex items-center gap-2">
                    <span class="text-sm font-semibold" style={{ color: "var(--text)" }}>Download Additional Model</span>
                    <Show when={modelPath() && modelPath().includes("ggml-")}>
                      <span class="text-[10px] font-semibold px-1.5 py-0.5 rounded-full bg-[var(--accent)] text-[var(--accent-fg)]">Active</span>
                    </Show>
                  </div>
                  <span class="text-[12px] mt-1" style={{ color: "var(--text-muted)" }}>
                    Current: {getCurrentModelName()}
                  </span>
                </div>
                
                {/* Model Selector Dropdown */}
                <div class="relative" ref={modelDropdownRef}>
                  <button
                    onClick={() => setShowModelDropdown(!showModelDropdown())}
                    disabled={isDownloading()}
                    class="flex items-center gap-2 px-3 py-1.5 rounded-xl text-xs font-semibold transition-all"
                    style={{ background: "var(--bg-surface2)", color: "var(--text)" }}
                  >
                    <span>{selectedModel()?.name ?? "Select"}</span>
                    <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
                    </svg>
                  </button>
                  
                  {/* Dropdown Menu */}
                  <Show when={showModelDropdown()}>
                    <div 
                      class="absolute right-0 top-full mt-1 w-72 max-h-80 z-50 rounded-xl border overflow-y-auto"
                      style={{ background: "var(--bg-card)", "border-color": "var(--border-soft)" }}
                    >
                      <For each={availableModels()}>
                        {(model) => (
                          <button
                            onClick={() => selectModel(model.id)}
                            class="w-full flex flex-col text-left px-4 py-3 transition-colors hover:bg-[var(--bg-surface)]"
                            style={{ 
                              background: selectedModelId() === model.id ? "var(--bg-surface)" : "transparent"
                            }}
                          >
                            <div class="flex items-center justify-between">
                              <span class="text-sm font-semibold" style={{ color: "var(--text)" }}>{model.name}</span>
                              <Show when={isCurrentModel(model.id)}>
                                <span class="text-[10px] font-semibold px-1.5 py-0.5 rounded-full bg-[var(--accent)] text-[var(--accent-fg)]">Active</span>
                              </Show>
                            </div>
                            <span class="text-[11px] mt-0.5" style={{ color: "var(--text-muted)" }}>{model.size_mb} MB • {model.description}</span>
                          </button>
                        )}
                      </For>
                    </div>
                  </Show>
                </div>
              </div>
              
              {/* Download Button and Progress */}
              <div class="flex items-center gap-3 px-4 pb-4">
                <button
                  onClick={downloadSelectedModel}
                  disabled={isDownloading() || isCurrentModel(selectedModelId())}
                  class="flex-1 flex items-center justify-center gap-2 px-4 py-2 rounded-xl text-sm font-semibold transition-all disabled:opacity-50"
                  style={{ background: "var(--accent)", color: "var(--accent-fg)" }}
                >
                  <Show when={isDownloading()} fallback={
                    <Show when={!isCurrentModel(selectedModelId())} fallback="Using">
                      <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" />
                      </svg>
                      Download ({selectedModel()?.size_mb} MB)
                    </Show>
                  }>
                    <svg class="w-4 h-4 animate-pulse" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" />
                    </svg>
                    Downloading {downloadProgress()}%
                  </Show>
                </button>
              </div>
            </div>

            {/* Custom File Browser Option */}
            <div class="flex items-center justify-between px-4 py-3.5 gap-3">
              <div class="flex flex-col gap-0.5 min-w-0">
                <div class="flex items-center gap-2">
                  <span class="text-sm font-semibold" style={{ color: "var(--text)" }}>Custom Local Model</span>
                  {modelPath() && !modelPath().includes("ggml-") && (
                    <span class="text-[10px] font-semibold px-1.5 py-0.5 rounded-full bg-[var(--accent)] text-[var(--accent-fg)]">Active</span>
                  )}
                </div>
                {modelPath() && !modelPath().includes("ggml-") && (
                  <span class="text-[12px] truncate" style={{ color: "var(--text-muted)" }}>{modelPath()}</span>
                )}
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
