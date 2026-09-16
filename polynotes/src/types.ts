export interface TranscriptEntry {
  id: string;
  timestamp: string; // "HH:MM:SS"
  text: string;
  /** 0..1, undefined when confidence extraction was off for this segment. */
  confidence?: number;
  isLowConfidence?: boolean;
  /** e.g. "en", "hi" — undefined when language detection was off. */
  language?: string;
}

export interface Flashcard {
  question: string;
  answer: string;
}

export interface GeneratedNotes {
  markdown: string;
  flashcards: Flashcard[];
}

export interface Note {
  id: string;
  title: string;
  body: string;
  createdAt: string;
  updatedAt: string;
  folderId?: string;
  transcripts: TranscriptEntry[];
  /** Cached result of the last "Generate Notes" call, if any. */
  generatedNotes?: GeneratedNotes;
}

export interface Folder {
  id: string;
  name: string;
  createdAt: string;
}

export type AudioSource = "microphone" | "app-audio";

/** Shape of the `transcription_segment` Tauri event payload. */
export interface TranscriptSegmentPayload {
  text: string;
  source: string;
  confidence?: number;
  is_low_confidence: boolean;
  language?: string;
}

