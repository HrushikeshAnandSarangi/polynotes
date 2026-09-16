# Polynotes web

A standalone Next.js download gallery for Polynotes releases. Not part of the Cargo workspace and not wired into the main build/release CI.

Windows and Linux download links are fetched live from the GitHub Releases API (`lib/github.ts`) — never hardcoded. macOS and Android always render as "coming soon," regardless of what the API returns, since CI does not build those platforms yet.

## Develop

```bash
npm install
npm run dev
```

## Build

```bash
npm run build
npm run start
```
