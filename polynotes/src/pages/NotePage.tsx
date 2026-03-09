import { createSignal, For, Show, onCleanup } from "solid-js";
import type { Accessor } from "solid-js";
import type { AudioSource, TranscriptEntry, Note } from "../types";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getFolders, moveNoteToFolder } from "../store";

interface Props {
  note: Accessor<Note | undefined>;
  onBack: () => void;
  onTranscript: (entry: TranscriptEntry) => void;
}

function nowTimestamp(): string {
  return new Date().toLocaleTimeString("en-US", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  });
}

function generateId() {
  return `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

type RecordState = "idle" | "recording" | "paused";

export function NotePage(props: Props) {
  const [source, setSource] = createSignal<AudioSource>("microphone");
  const [isStarting, setIsStarting] = createSignal(false);
  const [recordState, setRecordState] = createSignal<RecordState>("idle");
  const [isFolderMenuOpen, setIsFolderMenuOpen] = createSignal(false);
  
  let unlisten: (() => void)[] = [];
  let transcriptEndRef!: HTMLDivElement;

  onCleanup(() => {
    console.log("[polynotes] NotePage: onCleanup triggered. state =", recordState());
    if (recordState() !== "idle") {
      console.log("[polynotes] NotePage: stopping backend from onCleanup");
      invoke("stop_transcription").catch(() => {});
    }
    unlisten.forEach((fn) => fn());
    unlisten = [];
  });

  async function startRecording() {
    if (recordState() === "recording" || isStarting()) return;
    
    console.log("[polynotes] NotePage: startRecording clicked. source =", source());
    setIsStarting(true);
    setRecordState("recording");

    if (unlisten.length === 0) {
      console.log("[polynotes] NotePage: setting up transcription listeners");
      const u1 = await listen<string>("transcription_segment", (event) => {
        props.onTranscript({ id: generateId(), timestamp: nowTimestamp(), text: event.payload });
        setTimeout(() => transcriptEndRef?.scrollIntoView({ behavior: "smooth" }), 50);
      });
      const u2 = await listen<string>("transcription_error", (event) => {
        console.error("[polynotes] NotePage: received transcription_error event:", event.payload);
        alert(event.payload);
        setRecordState("idle");
      });
      unlisten = [u1, u2];
    }
    
    try {
      console.log("[polynotes] NotePage: invoking start_transcription");
      await invoke("start_transcription", { source: source() });
    } catch (e) {
      console.error("[polynotes] NotePage: start_transcription failed:", e);
      setRecordState("idle"); 
      alert(`Backend failed to start: ${e}`);
    } finally {
      setIsStarting(false);
    }
  }

  async function pauseRecording() {
    if (recordState() !== "recording") return;
    
    console.log("[polynotes] NotePage: pauseRecording clicked");
    setRecordState("paused");
    
    invoke("stop_transcription").catch((e) => {
      console.error("[polynotes] NotePage: stop_transcription (pause) failed:", e);
    });
  }

  async function endRecording() {
    if (recordState() === "idle") return;
    console.log("[polynotes] NotePage: endRecording clicked");
    setRecordState("idle");
    invoke("stop_transcription").catch((e) => console.error("[polynotes] NotePage: stop_transcription (end) failed:", e));
    unlisten.forEach((fn) => fn());
    unlisten = [];
  }

  const note = () => props.note();
  const transcripts = () => note()?.transcripts ?? [];

  const handleFolderSelect = (folderId?: string) => {
    const activeNote = note();
    if (activeNote) {
      moveNoteToFolder(activeNote.id, folderId);
    }
    setIsFolderMenuOpen(false);
  };

  const getFolderName = () => {
    const fId = note()?.folderId;
    if (!fId) return "Uncategorized";
    return getFolders()().find(f => f.id === fId)?.name ?? "Uncategorized";
  };

  return (
    <div class="min-h-screen flex flex-col" style={{ background: "var(--bg)" }}>

      {/* ── Sticky top nav ── */}
      <header
        class="sticky top-0 z-10 flex items-center gap-3 px-4 sm:px-6 pt-10 pb-3 max-w-3xl mx-auto w-full"
        style={{ background: "var(--bg)" }}
      >
        <button
          onClick={props.onBack}
          class="flex items-center justify-center w-9 h-9 rounded-xl flex-shrink-0"
          style={{ background: "var(--bg-surface2)", color: "var(--text-muted)" }}
          title="Back"
        >
          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.2" d="M15 19l-7-7 7-7" />
          </svg>
        </button>

        <div class="flex-1 min-w-0">
          <input
            type="text"
            value={note()?.title ?? ""}
            onBlur={(e) => {
              const newTitle = e.currentTarget.value.trim();
              if (newTitle && note()) {
                import("../store").then((m) => m.updateNote(note()!.id, { title: newTitle }));
              } else if (note()) {
                e.currentTarget.value = note()!.title;
              }
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.currentTarget.blur();
              }
            }}
            class="w-full text-sm font-semibold truncate bg-transparent outline-none border-none p-0 transition-colors focus:bg-[var(--bg-surface2)] focus:px-2 focus:-ml-2 rounded-sm"
            style={{ color: "var(--text)" }}
            placeholder="Untitled Note"
          />
          <div class="flex flex-wrap items-center gap-2 mt-0.5 truncate">
            <span class="text-[11px]" style={{ color: "var(--text-subtle)" }}>
              {transcripts().length} {transcripts().length === 1 ? "segment" : "segments"}
            </span>
            <Show when={recordState() === "recording"}>
              <span class="flex items-center gap-1 text-[11px] whitespace-nowrap" style={{ color: "var(--red)" }}>
                <span class="w-1.5 h-1.5 rounded-full animate-pulse inline-block shrink-0" style={{ background: "var(--red)" }} />
                Recording
              </span>
            </Show>
            <Show when={recordState() === "paused"}>
              <span class="text-[11px] whitespace-nowrap" style={{ color: "var(--amber)" }}>Paused</span>
            </Show>
          </div>
        </div>

        {/* Folder Selector Menu Button */}
        <div class="relative shrink-0">
          <button
            onClick={() => setIsFolderMenuOpen(!isFolderMenuOpen())}
            class="flex items-center gap-1.5 px-3 py-1.5 rounded-xl text-xs font-semibold"
            style={{ background: "var(--bg-surface2)", color: "var(--text-muted)", border: "1px solid var(--border-soft)" }}
          >
            <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8"
                    d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
            </svg>
            <span class="max-w-[70px] truncate">{getFolderName()}</span>
            <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
            </svg>
          </button>
          
          <Show when={isFolderMenuOpen()}>
            <>
              {/* Backing dismiss overlay */}
              <div 
                class="fixed inset-0 z-40" 
                onClick={() => setIsFolderMenuOpen(false)}
              />
              <div 
                class="absolute right-0 mt-2 w-48 rounded-2xl shadow-lg border p-1 z-50 text-left"
                style={{ background: "var(--bg-card)", "border-color": "var(--border)" }}
              >
                <button
                  onClick={() => handleFolderSelect(undefined)}
                  class="w-full text-left px-3 py-2 text-sm rounded-xl transition-colors hover:bg-[var(--bg-surface)]"
                  style={{ color: "var(--text)" }}
                >
                  Clear / Uncategorized
                </button>
                <div class="h-px w-full my-1" style={{ background: "var(--border-soft)" }} />
                <For each={getFolders()()}>
                  {(folder) => (
                    <button
                      onClick={() => handleFolderSelect(folder.id)}
                      class="w-full text-left px-3 py-2 text-sm rounded-xl transition-colors hover:bg-[var(--bg-surface)] flex justify-between items-center"
                      style={{ color: "var(--text)" }}
                    >
                      <span class="truncate">{folder.name}</span>
                      <Show when={note()?.folderId === folder.id}>
                        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24" style={{ color: "var(--accent)" }}>
                           <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M5 13l4 4L19 7" />
                        </svg>
                      </Show>
                    </button>
                  )}
                </For>
              </div>
            </>
          </Show>
        </div>
      </header>

      {/* ── Divider ── */}
      <div class="mx-4 sm:mx-6 h-px max-w-3xl mx-auto w-full" style={{ background: "var(--border-soft)" }} />

      {/* ── Transcript canvas ── */}
      <main class="flex-1 overflow-y-auto px-4 sm:px-6 py-6" style={{ "padding-bottom": "240px" }}>
        <Show
          when={transcripts().length > 0}
          fallback={
            <div class="flex flex-col items-center justify-center mt-24 gap-4 text-center select-none">
              <div
                class="w-16 h-16 rounded-2xl flex items-center justify-center"
                style={{ background: "var(--bg-surface2)" }}
              >
                <svg class="w-7 h-7" fill="none" stroke="var(--text-subtle)" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.4"
                    d="M19 11a7 7 0 01-7 7m0 0a7 7 0 01-7-7m7 7v4m0 0H8m4 0h4m-4-8a3 3 0 01-3-3V5a3 3 0 116 0v6a3 3 0 01-3 3z" />
                </svg>
              </div>
              <div>
                <p class="text-sm font-semibold" style={{ color: "var(--text)" }}>
                  {recordState() === "idle" ? "Ready to record" : "Listening…"}
                </p>
                <p class="text-xs mt-1" style={{ color: "var(--text-muted)" }}>
                  {recordState() === "idle"
                    ? "Choose an audio source and press record."
                    : "Transcription segments will appear here."}
                </p>
              </div>
            </div>
          }
        >
          <div class="max-w-2xl mx-auto space-y-1 pb-4">
            <For each={transcripts()}>
              {(entry) => (
                <div
                  class="flex gap-4 items-start py-3 px-1 rounded-xl transition-colors"
                >
                  {/* Time badge */}
                  <span
                    class="shrink-0 text-[11px] font-mono pt-1 tabular-nums mt-1 select-none w-16 text-right opacity-60"
                    style={{ color: "var(--text-subtle)" }}
                  >
                    {entry.timestamp}
                  </span>
                  {/* Text */}
                  <p class="flex-1 text-[16px] leading-[1.7]" style={{ color: "var(--text)" }}>
                    {entry.text}
                  </p>
                </div>
              )}
            </For>
            {/* Scroll anchor */}
            <div ref={transcriptEndRef!} />
          </div>
        </Show>
      </main>

      {/* ── Bottom controls ── */}
      <div
        class="fixed bottom-0 left-0 right-0 border-t"
        style={{ background: "var(--bg)", "border-color": "var(--border-soft)" }}
      >
        <div class="flex flex-col items-center gap-4 pt-4 pb-8 px-6 max-w-3xl mx-auto w-full">

          {/* Source toggle */}
          <div
            class="flex items-center rounded-2xl p-1 gap-1"
            style={{ background: "var(--bg-surface2)" }}
          >
            <button
              onClick={() => setSource("microphone")}
              disabled={recordState() === "recording"}
              class="flex items-center gap-2 px-4 py-2 rounded-xl text-xs font-medium transition-all disabled:opacity-40 disabled:cursor-not-allowed"
              style={
                source() === "microphone"
                  ? { background: "var(--bg-card)", color: "var(--text)", "box-shadow": "0 1px 4px rgba(0,0,0,0.08)" }
                  : { background: "transparent", color: "var(--text-muted)" }
              }
            >
              <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                  d="M19 11a7 7 0 01-7 7m0 0a7 7 0 01-7-7m7 7v4m0 0H8m4 0h4m-4-8a3 3 0 01-3-3V5a3 3 0 116 0v6a3 3 0 01-3 3z" />
              </svg>
              Voice Input
            </button>
            <button
              onClick={() => setSource("app-audio")}
              disabled={recordState() === "recording"}
              class="flex items-center gap-2 px-4 py-2 rounded-xl text-xs font-medium transition-all disabled:opacity-40 disabled:cursor-not-allowed"
              style={
                source() === "app-audio"
                  ? { background: "var(--bg-card)", color: "var(--text)", "box-shadow": "0 1px 4px rgba(0,0,0,0.08)" }
                  : { background: "transparent", color: "var(--text-muted)" }
              }
            >
              <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                  d="M9 19V6l12-3v13M9 19c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zm12-3c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zM9 10l12-3" />
              </svg>
              App Audio
            </button>
          </div>

          {/* Transport controls */}
          <div class="relative flex items-center justify-center w-full mt-2 h-24">
            {/* Stop Button (Absolutely positioned to left so Play button is pure center) */}
            <div class="absolute left-2 sm:left-10 transition-opacity duration-300" style={{ opacity: recordState() === "idle" ? "0" : "1", "pointer-events": recordState() === "idle" ? "none" : "auto" }}>
              <button
                onClick={endRecording}
                disabled={recordState() === "idle"}
                class="w-12 h-12 sm:w-14 sm:h-14 rounded-2xl flex items-center justify-center transition-all duration-300 active:scale-90 hover:scale-105"
                style={{ background: "var(--red-soft)", color: "var(--red)" }}
                title="Stop and Save"
              >
                <svg class="w-5 h-5 sm:w-6 sm:h-6" fill="currentColor" viewBox="0 0 24 24">
                  <rect x="6" y="6" width="12" height="12" rx="2" />
                </svg>
              </button>
            </div>

            {/* Play / Pause Toggle Button */}
            <button
              onClick={recordState() === "recording" ? pauseRecording : startRecording}
              class="w-16 h-16 sm:w-20 sm:h-20 rounded-3xl flex items-center justify-center shadow-lg transition-all duration-300 active:scale-95 hover:scale-105 hover:shadow-xl z-10"
              style={
                recordState() === "recording"
                  ? { background: "var(--amber)", color: "#fff" }
                  : { background: "var(--accent)", color: "var(--accent-fg)" }
              }
              title={recordState() === "recording" ? "Pause" : recordState() === "paused" ? "Resume" : "Play"}
            >
              <svg 
                class={`w-6 h-6 sm:w-8 sm:h-8 transition-transform duration-300 ${recordState() !== "recording" ? "translate-x-0.5" : ""}`} 
                fill="currentColor" 
                viewBox="0 0 24 24"
              >
                {recordState() === "recording" ? (
                  <path d="M6 5h4v14H6V5zm8 0h4v14h-4V5z" />
                ) : (
                  <path d="M8 5v14l11-7z" />
                )}
              </svg>
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
