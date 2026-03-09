export interface TranscriptEntry {
  id: string;
  timestamp: string; // "HH:MM:SS"
  text: string;
}

export interface Note {
  id: string;
  title: string;
  body: string;
  createdAt: string;
  updatedAt: string;
  folderId?: string;
  transcripts: TranscriptEntry[];
}

export interface Folder {
  id: string;
  name: string;
  createdAt: string;
}

export type AudioSource = "microphone" | "app-audio";

