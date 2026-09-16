import { createSignal, Show, For } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import { marked } from "marked";
import type { GeneratedNotes, TranscriptEntry } from "../types";
import { GEMINI_API_KEY_KEY } from "./SettingsPage";

interface Props {
  title: string;
  transcripts: TranscriptEntry[];
  generatedNotes?: GeneratedNotes;
  onGenerated: (notes: GeneratedNotes) => void;
  onClose: () => void;
}

type ExportFormat = "markdown" | "pdf" | "anki";

export function NotesPanel(props: Props) {
  const [isGenerating, setIsGenerating] = createSignal(false);
  const [error, setError] = createSignal<string | undefined>(undefined);
  const [exportingFormat, setExportingFormat] = createSignal<ExportFormat | undefined>(undefined);

  const apiKey = () => localStorage.getItem(GEMINI_API_KEY_KEY) ?? "";
  const notes = () => props.generatedNotes;
  const renderedHtml = () => (notes() ? marked.parse(notes()!.markdown, { async: false }) : "");

  async function generate() {
    if (!apiKey()) {
      setError("Add a Gemini API key in Settings first.");
      return;
    }
    const transcript = props.transcripts.map((t) => t.text).join("\n");
    if (!transcript.trim()) {
      setError("Nothing to summarize yet — record a session first.");
      return;
    }

    setIsGenerating(true);
    setError(undefined);
    try {
      const result = await invoke<GeneratedNotes>("generate_notes_cmd", {
        transcript,
        apiKey: apiKey(),
      });
      props.onGenerated(result);
    } catch (e) {
      setError(`Failed to generate notes: ${e}`);
    } finally {
      setIsGenerating(false);
    }
  }

  async function exportAs(format: ExportFormat) {
    const current = notes();
    if (!current) return;

    const filters =
      format === "markdown"
        ? [{ name: "Markdown", extensions: ["md"] }]
        : format === "pdf"
        ? [{ name: "PDF", extensions: ["pdf"] }]
        : [{ name: "CSV", extensions: ["csv"] }];
    const defaultExt = format === "markdown" ? "md" : format === "pdf" ? "pdf" : "csv";

    const destPath = await saveDialog({
      title: `Export ${format === "anki" ? "Anki flashcards" : format.toUpperCase()}`,
      defaultPath: `${props.title || "notes"}.${defaultExt}`,
      filters,
    });
    if (!destPath) return;

    setExportingFormat(format);
    try {
      if (format === "markdown") {
        await invoke("export_markdown", { title: props.title, markdownBody: current.markdown, destPath });
      } else if (format === "pdf") {
        await invoke("export_pdf", { title: props.title, markdownBody: current.markdown, destPath });
      } else {
        await invoke("export_anki_csv", { flashcards: current.flashcards, destPath });
      }
    } catch (e) {
      setError(`Export failed: ${e}`);
    } finally {
      setExportingFormat(undefined);
    }
  }

  return (
    <div class="fixed inset-0 z-50 flex items-end sm:items-center justify-center">
      <div class="absolute inset-0" style={{ background: "rgba(0,0,0,0.4)" }} onClick={props.onClose} />
      <div
        class="relative w-full sm:max-w-2xl sm:rounded-2xl rounded-t-2xl max-h-[85vh] flex flex-col border overflow-hidden"
        style={{ background: "var(--bg)", "border-color": "var(--border)" }}
      >
        <header
          class="flex items-center justify-between px-5 py-4 border-b shrink-0"
          style={{ "border-color": "var(--border-soft)" }}
        >
          <span class="text-sm font-semibold" style={{ color: "var(--text)" }}>Notes &amp; Flashcards</span>
          <button
            onClick={props.onClose}
            class="w-8 h-8 rounded-lg flex items-center justify-center"
            style={{ background: "var(--bg-surface2)", color: "var(--text-muted)" }}
          >
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </header>

        <div class="flex-1 overflow-y-auto px-5 py-4">
          <Show when={error()}>
            <div
              class="mb-4 px-3 py-2 rounded-xl text-xs"
              style={{ background: "var(--red-soft)", color: "var(--red)" }}
            >
              {error()}
            </div>
          </Show>

          <Show
            when={notes()}
            fallback={
              <div class="flex flex-col items-center justify-center gap-3 py-10 text-center">
                <p class="text-sm" style={{ color: "var(--text-muted)" }}>
                  Turn this transcript into structured notes and flashcards using Gemini.
                </p>
                <button
                  onClick={generate}
                  disabled={isGenerating()}
                  class="px-4 py-2 rounded-xl text-sm font-semibold disabled:opacity-50"
                  style={{ background: "var(--accent)", color: "var(--accent-fg)" }}
                >
                  {isGenerating() ? "Generating…" : "Generate Notes"}
                </button>
              </div>
            }
          >
            <div class="space-y-6">
              <div
                class="prose-notes text-sm leading-relaxed"
                style={{ color: "var(--text)" }}
                innerHTML={renderedHtml() as string}
              />

              <Show when={notes()!.flashcards.length > 0}>
                <div>
                  <p class="text-[11px] font-semibold uppercase tracking-widest mb-2" style={{ color: "var(--text-subtle)" }}>
                    Flashcards ({notes()!.flashcards.length})
                  </p>
                  <div class="space-y-2">
                    <For each={notes()!.flashcards}>
                      {(card) => (
                        <div class="rounded-xl border p-3" style={{ "border-color": "var(--border-soft)", background: "var(--bg-card)" }}>
                          <p class="text-sm font-medium" style={{ color: "var(--text)" }}>{card.question}</p>
                          <p class="text-sm mt-1" style={{ color: "var(--text-muted)" }}>{card.answer}</p>
                        </div>
                      )}
                    </For>
                  </div>
                </div>
              </Show>

              <button
                onClick={generate}
                disabled={isGenerating()}
                class="text-xs font-semibold underline disabled:opacity-50"
                style={{ color: "var(--text-muted)" }}
              >
                {isGenerating() ? "Regenerating…" : "Regenerate"}
              </button>
            </div>
          </Show>
        </div>

        <Show when={notes()}>
          <footer class="flex gap-2 px-5 py-4 border-t shrink-0" style={{ "border-color": "var(--border-soft)" }}>
            <button
              onClick={() => exportAs("markdown")}
              disabled={exportingFormat() !== undefined}
              class="flex-1 py-2 rounded-xl text-xs font-semibold disabled:opacity-50"
              style={{ background: "var(--bg-surface2)", color: "var(--text)" }}
            >
              {exportingFormat() === "markdown" ? "Exporting…" : "Markdown"}
            </button>
            <button
              onClick={() => exportAs("pdf")}
              disabled={exportingFormat() !== undefined}
              class="flex-1 py-2 rounded-xl text-xs font-semibold disabled:opacity-50"
              style={{ background: "var(--bg-surface2)", color: "var(--text)" }}
            >
              {exportingFormat() === "pdf" ? "Exporting…" : "PDF"}
            </button>
            <button
              onClick={() => exportAs("anki")}
              disabled={exportingFormat() !== undefined}
              class="flex-1 py-2 rounded-xl text-xs font-semibold disabled:opacity-50"
              style={{ background: "var(--bg-surface2)", color: "var(--text)" }}
            >
              {exportingFormat() === "anki" ? "Exporting…" : "Anki CSV"}
            </button>
          </footer>
        </Show>
      </div>
    </div>
  );
}
