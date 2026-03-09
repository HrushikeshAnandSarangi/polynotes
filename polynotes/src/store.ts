import { createSignal } from "solid-js";
import type { Note, TranscriptEntry, Folder } from "./types";

const NOTES_KEY = "polynotes_notes";
const FOLDERS_KEY = "polynotes_folders";

function loadData<T>(key: string): T[] {
  try {
    const raw = localStorage.getItem(key);
    return raw ? (JSON.parse(raw) as T[]) : [];
  } catch {
    return [];
  }
}

function saveData<T>(key: string, data: T[]) {
  localStorage.setItem(key, JSON.stringify(data));
}

function nowISO() {
  return new Date().toISOString();
}

function generateId() {
  return `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

const [notes, setNotes] = createSignal<Note[]>(loadData(NOTES_KEY));
const [folders, setFolders] = createSignal<Folder[]>(loadData(FOLDERS_KEY));

function persistNotes(updated: Note[]) {
  saveData(NOTES_KEY, updated);
  setNotes(updated);
}

function persistFolders(updated: Folder[]) {
  saveData(FOLDERS_KEY, updated);
  setFolders(updated);
}

// ── Notes API ──
export function getNotes() { return notes; }

export function createNote(): Note {
  const now = new Date();
  const title = now.toLocaleDateString("en-US", { month: "short", day: "numeric" }) + 
    " · " + now.toLocaleTimeString("en-US", { hour: "2-digit", minute: "2-digit", hour12: true });
  
  const note: Note = {
    id: generateId(),
    title,
    body: "",
    createdAt: now.toISOString(),
    updatedAt: now.toISOString(),
    transcripts: [],
  };
  persistNotes([note, ...notes()]);
  return note;
}

export function updateNote(id: string, patch: Partial<Pick<Note, "title" | "body">>) {
  persistNotes(notes().map((n) => n.id === id ? { ...n, ...patch, updatedAt: nowISO() } : n));
}

export function moveNoteToFolder(noteId: string, folderId?: string) {
  persistNotes(notes().map((n) => n.id === noteId ? { ...n, folderId, updatedAt: nowISO() } : n));
}

export function deleteNote(id: string) {
  persistNotes(notes().filter((n) => n.id !== id));
}

export function appendTranscript(noteId: string, entry: TranscriptEntry) {
  persistNotes(notes().map((n) => n.id === noteId ? { ...n, transcripts: [...n.transcripts, entry] } : n));
}

export function clearAllNotes() {
  persistNotes([]);
}

// ── Folders API ──
export function getFolders() { return folders; }

export function createFolder(name: string): Folder {
  const folder: Folder = {
    id: generateId(),
    name,
    createdAt: nowISO(),
  };
  persistFolders([...folders(), folder]);
  return folder;
}

export function deleteFolder(id: string) {
  // Move all notes in this folder back to un-categorized
  persistNotes(notes().map(n => n.folderId === id ? { ...n, folderId: undefined } : n));
  persistFolders(folders().filter((f) => f.id !== id));
}
