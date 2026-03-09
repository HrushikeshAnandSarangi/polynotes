import { createSignal, createMemo, For, Show } from "solid-js";
import type { Accessor } from "solid-js";
import type { Note } from "../types";
import { getFolders, createFolder, deleteFolder } from "../store";

interface Props {
  notes: Accessor<Note[]>;
  onCreate: (folderId?: string) => void;
  onOpen: (id: string) => void;
  onDelete: (id: string) => void;
  onSettings: () => void;
}

function formatMinimalDate(iso: string): string {
  const d = new Date(iso);
  const now = new Date();
  
  if (d.getFullYear() === now.getFullYear()) {
    return d.toLocaleDateString("en-US", { month: "long", day: "numeric" });
  }
  return d.toLocaleDateString("en-US", { month: "long", day: "numeric", year: "numeric" });
}

export function HomePage(props: Props) {
  const [activeFolderId, setActiveFolderId] = createSignal<string | undefined>(undefined);
  const [searchQuery, setSearchQuery] = createSignal<string>("");
  const [itemToDelete, setItemToDelete] = createSignal<{ type: "folder" | "note"; id: string; name: string } | null>(null);

  let dialogRef!: HTMLDialogElement;
  let deleteDialogRef!: HTMLDialogElement;
  let inputRef!: HTMLInputElement;

  const filteredNotes = createMemo(() => {
    const q = searchQuery().toLowerCase();
    const folderId = activeFolderId();
    return props.notes().filter(n => {
      // 1. Folder match
      if (folderId && n.folderId !== folderId) return false;
      // 2. Search match
      if (!q) return true;
      const matchesTitle = n.title.toLowerCase().includes(q);
      const matchesTranscripts = n.transcripts.some(t => t.text.toLowerCase().includes(q));
      return matchesTitle || matchesTranscripts;
    });
  });

  const handleCreateFolder = (e: Event) => {
    e.preventDefault();
    const name = inputRef.value.trim();
    if (name) {
      const folder = createFolder(name);
      setActiveFolderId(folder.id);
      inputRef.value = "";
      dialogRef.close();
    }
  };

  const confirmDelete = () => {
    const target = itemToDelete();
    if (!target) return;

    if (target.type === "folder") {
      deleteFolder(target.id);
      if (activeFolderId() === target.id) setActiveFolderId(undefined);
    } else if (target.type === "note") {
      props.onDelete(target.id);
    }
    
    closeDeleteDialog();
  };

  const closeDeleteDialog = () => {
    setItemToDelete(null);
    deleteDialogRef.close();
  };

  return (
    <div
      class="min-h-screen flex flex-col relative"
      style={{ background: "var(--bg)", "padding-bottom": "40px" }}
    >
      {/* ── Top nav ── */}
      <header
        class="sticky top-0 z-10 w-full pt-4 sm:pt-8 pb-3 sm:pb-4"
        style={{ background: "var(--bg)" }}
      >
        <div class="max-w-6xl mx-auto px-5 sm:px-8">
          {/* Top Actions Row */}
          <div class="flex items-center justify-end gap-3 mb-3 sm:mb-4">
            <button
              onClick={() => dialogRef.showModal()}
              class="w-9 h-9 flex items-center justify-center transition-colors active:scale-95"
              style={{ color: "var(--text)" }}
              title="New Folder"
            >
              <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5"
                  d="M9 13h6m-3-3v6m-9 1V7a2 2 0 012-2h4l2 2h4a2 2 0 012 2v1m-6-4v1m0 0H9m6 0v1" />
              </svg>
            </button>
            <button
              onClick={props.onSettings}
              class="w-9 h-9 flex items-center justify-center transition-colors active:scale-95"
              style={{ color: "var(--text)" }}
              title="Settings"
            >
              <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5"
                  d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
              </svg>
            </button>
          </div>

          {/* Large Title */}
          <h1 class="text-3xl sm:text-4xl font-normal tracking-tight mb-4 sm:mb-5" style={{ color: "var(--text)" }}>
            Polynotes
          </h1>

          {/* Search Bar */}
          <div class="relative w-full mb-5">
            <div class="absolute inset-y-0 left-0 pl-4 flex items-center pointer-events-none">
              <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24" style={{ color: "var(--text-muted)" }}>
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
              </svg>
            </div>
            <input
              type="text"
              placeholder="Search notes by title or content"
              value={searchQuery()}
              onInput={e => setSearchQuery(e.currentTarget.value)}
              class="w-full pl-11 pr-4 py-3 rounded-full text-[15px] outline-none transition-colors border-2 border-transparent focus:border-[var(--accent)]"
              style={{ background: "var(--bg-surface2)", color: "var(--text)" }}
            />
          </div>

          {/* Folder Tabs */}
          <div class="flex items-center gap-2 overflow-x-auto pb-2 scrollbar-none w-full" style={{ "scrollbar-width": "none" }}>
            <button
              onClick={() => setActiveFolderId(undefined)}
              class="whitespace-nowrap px-4 py-1.5 rounded-full text-sm font-medium transition-colors"
              style={activeFolderId() === undefined 
                ? { background: "var(--text)", color: "var(--bg)" } 
                : { background: "transparent", color: "var(--text-muted)", border: "1px solid var(--border)" }}
            >
              All Notes
            </button>
            <For each={getFolders()()}>
              {(folder) => (
                <button
                  onClick={() => setActiveFolderId(folder.id)}
                  class="whitespace-nowrap px-4 py-1.5 rounded-full text-sm font-medium transition-colors group flex items-center gap-2"
                  style={activeFolderId() === folder.id 
                    ? { background: "var(--text)", color: "var(--bg)" } 
                    : { background: "transparent", color: "var(--text-muted)", border: "1px solid var(--border)" }}
                >
                  {folder.name}
                  <div
                    onClick={(e) => { 
                      e.stopPropagation(); 
                      setItemToDelete({ type: "folder", id: folder.id, name: folder.name });
                      deleteDialogRef.showModal();
                    }}
                    class="ml-1 opacity-0 group-hover:opacity-100 transition-opacity"
                    title="Delete folder"
                  >
                    <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                       <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M6 18L18 6M6 6l12 12" />
                    </svg>
                  </div>
                </button>
              )}
            </For>
          </div>
        </div>
      </header>

      {/* ── Notes content ── */}
      <main class="flex-1 w-full max-w-6xl mx-auto px-5 sm:px-8 pb-10">
        <Show
          when={filteredNotes().length > 0}
          fallback={
            <div class="flex flex-col items-center justify-center mt-24 gap-5 text-center select-none">
              <div
                class="w-20 h-20 rounded-3xl flex items-center justify-center"
                style={{ background: "var(--bg-surface2)" }}
              >
                <svg class="w-9 h-9" fill="none" stroke="var(--text-subtle)" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.3"
                    d="M19 11a7 7 0 01-7 7m0 0a7 7 0 01-7-7m7 7v4m0 0H8m4 0h4m-4-8a3 3 0 01-3-3V5a3 3 0 116 0v6a3 3 0 01-3 3z" />
                </svg>
              </div>
              <div>
                <p class="text-base font-semibold" style={{ color: "var(--text)" }}>Empty Folder</p>
                <p class="text-sm mt-1" style={{ color: "var(--text-muted)" }}>
                  Tap <strong>+</strong> to add a newly recorded note.
                </p>
              </div>
            </div>
          }
        >
          <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4 pt-2">
            <For each={filteredNotes()}>
              {(note) => (
                <div
                  onClick={() => props.onOpen(note.id)}
                  class="group relative cursor-pointer rounded-[24px] p-5 pb-4 text-left flex flex-col items-start transition-all duration-300 hover:-translate-y-1 active:translate-y-0 active:scale-[0.98]"
                  style={{
                    background: "var(--bg-card)",
                    "box-shadow": "var(--shadow-sm)",
                  }}
                >
                  {/* Title */}
                  <p class="text-base font-semibold line-clamp-1 mb-3 w-full border-b pb-2" style={{ color: "var(--text)", "border-color": "var(--border-soft)" }}>
                    {note.title}
                  </p>

                  {/* Preview text */}
                  <p class="text-[14px] line-clamp-4 leading-[1.6] mb-5 w-full font-medium" style={{ color: "var(--text-muted)" }}>
                    {note.transcripts.length > 0
                      ? note.transcripts[note.transcripts.length - 1].text
                      : "No text"}
                  </p>

                  {/* Footer Date */}
                  <div class="flex items-center justify-between w-full mt-auto">
                    <span class="text-[12px] font-medium" style={{ color: "var(--text-subtle)" }}>
                      {formatMinimalDate(note.updatedAt)}
                    </span>
                  </div>

                  {/* Delete (Hidden on mobile by default, shown on hover/focus) */}
                  <button
                    onClick={(e) => { 
                      e.stopPropagation(); 
                      setItemToDelete({ type: "note", id: note.id, name: note.title });
                      deleteDialogRef.showModal();
                    }}
                    class="absolute top-3 right-3 w-7 h-7 rounded-lg items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity hidden md:flex"
                    style={{ background: "var(--red-soft)", color: "var(--red)" }}
                    title="Delete"
                  >
                    <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                        d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                    </svg>
                  </button>

                  <div
                    class="absolute inset-0 rounded-[24px] opacity-0 group-hover:opacity-100 transition-opacity duration-300 pointer-events-none"
                    style={{ "box-shadow": "var(--shadow-md)" }}
                  />
                </div>
              )}
            </For>
          </div>
        </Show>
      </main>

      {/* ── Fixed FAB (Floating Action Button) ── */}
      <button
        onClick={() => props.onCreate(activeFolderId())}
        class="fixed bottom-8 right-6 md:right-10 w-[60px] h-[60px] rounded-full shadow-lg flex items-center justify-center z-20 active:scale-90 transition-transform hover:scale-105"
        style={{ 
          background: "#F5B031", // Matches the golden/yellow in image
          color: "#FFFFFF",
          "box-shadow": "0 4px 14px rgba(245, 176, 49, 0.4)" 
        }}
      >
        <svg class="w-8 h-8" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M12 4v16m8-8H4" />
        </svg>
      </button>

      {/* ── Create Folder Dialog ── */}
      <dialog
        ref={dialogRef!}
        class="rounded-3xl border p-6 max-w-xs w-[90%] md:w-full fixed top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 m-0 backdrop:bg-black/40 backdrop:backdrop-blur-sm shadow-2xl"
        style={{ background: "var(--bg-card)", "border-color": "var(--border)", color: "var(--text)" }}
      >
        <form onSubmit={handleCreateFolder}>
          <h3 class="text-lg font-semibold mb-2" style={{ color: "var(--text)" }}>
            New Folder
          </h3>
          <p class="text-sm mb-5 leading-relaxed" style={{ color: "var(--text-muted)" }}>
            Group related notes together.
          </p>
          <input
            ref={inputRef!}
            type="text"
            placeholder="Folder name"
            required
            class="w-full px-4 py-3 rounded-xl text-[15px] outline-none border-2 transition-colors focus:border-[var(--accent)] mb-5"
            style={{ background: "var(--bg-surface)", "border-color": "var(--border-soft)", color: "var(--text)" }}
          />
          <div class="flex gap-2">
            <button
              type="button"
              onClick={() => { dialogRef.close(); inputRef.value = ""; }}
              class="flex-1 py-2.5 rounded-xl border text-sm font-medium transition-colors hover:bg-[var(--bg-surface)]"
              style={{ "border-color": "var(--border)", color: "var(--text)" }}
            >
              Cancel
            </button>
            <button
              type="submit"
              class="flex-1 py-2.5 rounded-xl text-sm font-medium text-white transition-opacity hover:opacity-90"
              style={{ background: "var(--accent)", color: "var(--accent-fg)" }}
            >
              Create
            </button>
          </div>
        </form>
      </dialog>

      {/* ── Delete Confirmation Dialog ── */}
      <dialog
        ref={deleteDialogRef!}
        class="rounded-3xl border p-6 max-w-xs w-[90%] md:w-full fixed top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 m-0 backdrop:bg-black/40 backdrop:backdrop-blur-sm shadow-2xl"
        style={{ background: "var(--bg-card)", "border-color": "var(--border)", color: "var(--text)" }}
      >
        <div class="flex flex-col">
          <h3 class="text-lg font-semibold mb-2" style={{ color: "var(--text)" }}>
            Confirm Deletion
          </h3>
          <p class="text-sm mb-6 leading-relaxed" style={{ color: "var(--text-muted)" }}>
            Are you sure you want to delete the {itemToDelete()?.type} "<strong>{itemToDelete()?.name}</strong>"?
            <Show when={itemToDelete()?.type === "folder"}>
              <br /><br />
              Its notes will not be deleted, they will simply become uncategorized.
            </Show>
          </p>
          <div class="flex gap-2">
            <button
              type="button"
              onClick={closeDeleteDialog}
              class="flex-1 py-2.5 rounded-xl border text-sm font-medium transition-colors hover:bg-[var(--bg-surface)]"
              style={{ "border-color": "var(--border)", color: "var(--text)" }}
            >
              Cancel
            </button>
            <button
              type="button"
              onClick={confirmDelete}
              class="flex-1 py-2.5 rounded-xl text-sm font-medium transition-opacity hover:opacity-90"
              style={{ background: "var(--red)", color: "#fff" }}
            >
              Delete
            </button>
          </div>
        </div>
      </dialog>
    </div>
  );
}

