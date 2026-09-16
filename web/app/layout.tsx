import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "Polynotes — Download",
  description:
    "Download Polynotes, a real-time multilingual lecture transcription app, for your device.",
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
