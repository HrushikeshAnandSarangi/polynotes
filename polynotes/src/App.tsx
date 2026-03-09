import { createMemo, createSignal, onMount, Show } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { HomePage } from "./pages/HomePage";
import { NotePage } from "./pages/NotePage";
import { SettingsPage } from "./pages/SettingsPage";
import {
  getNotes,
  createNote,
  deleteNote,
  clearAllNotes,
  appendTranscript,
} from "./store";
import { getTheme } from "./theme";
import type { TranscriptEntry } from "./types";
import "./App.css";

const MODEL_PATH_KEY = "polynotes_model_path";

type Page = { name: "home" } | { name: "note"; id: string } | { name: "settings" };

function App() {
  const [page, setPage] = createSignal<Page>({ name: "home" });

  // Sync persisted model path to backend immediately on startup,
  // so recording works even if the user hasn't visited Settings yet.
  onMount(async () => {
    const saved = localStorage.getItem(MODEL_PATH_KEY) ?? "";
    if (saved) {
      await invoke("set_model_path", { path: saved });
    }
  });

  const activeNote = createMemo(() => {
    const p = page();
    if (p.name !== "note") return undefined;
    return getNotes()().find((n) => n.id === p.id);
  });

  function handleCreate(folderId?: string) {
    const note = createNote();
    if (folderId) {
      import("./store").then(m => m.moveNoteToFolder(note.id, folderId));
    }
    setPage({ name: "note", id: note.id });
  }

  function handleOpen(id: string) {
    setPage({ name: "note", id });
  }

  function handleDelete(id: string) {
    deleteNote(id);
    const p = page();
    if (p.name === "note" && (p as { name: "note"; id: string }).id === id) {
      setPage({ name: "home" });
    }
  }

  function handleTranscript(entry: TranscriptEntry) {
    const p = page();
    if (p.name === "note") appendTranscript(p.id, entry);
  }

  return (
    /* data-theme drives all CSS variable overrides */
    <div data-theme={getTheme()()} style={{ "min-height": "100vh", background: "var(--bg)" }}>
      <Show when={page().name === "home"}>
        <HomePage
          notes={getNotes()}
          onCreate={handleCreate}
          onOpen={handleOpen}
          onDelete={handleDelete}
          onSettings={() => setPage({ name: "settings" })}
        />
      </Show>
      <Show when={page().name === "note"}>
        <NotePage
          note={activeNote}
          onBack={() => setPage({ name: "home" })}
          onTranscript={handleTranscript}
        />
      </Show>
      <Show when={page().name === "settings"}>
        <SettingsPage
          onBack={() => setPage({ name: "home" })}
          onClearAll={() => { clearAllNotes(); setPage({ name: "home" }); }}
        />
      </Show>
    </div>
  );
}

export default App;
