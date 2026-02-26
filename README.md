# embd-IDE

A lightweight (~20MB) IDE that's written with a rust backend for speed and consistency. 

---

## Table of Contents
- [Prerequisites](#prerequisites)
- [Setup](#setup)
- [Run - Development](#run---development)
- [Build - Production App](#build---production-app)
- [Beta Testing](#beta-testing)
- [Themes](#themes)
- [Feedback](#feedback)

## How to get started: 

### Prerequisites: 
1. Node.js (v18+) — [Download](https://nodejs.org)
2. Rust
```bash
brew install rust
```
For other platforms, see [rustup.rs](https://rustup.rs)

3. Tauri system dependencies — Varies by OS: 
    - macOS: Xcode Command Line Tools
      ```bash 
      xcode-select --install
      ```                                                                                                                                                                 
    - Windows: Microsoft Visual Studio C++ Build Tools + WebView2                                                                                                                                            
    - Linux:
      ```bash 
      sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
      ```

### Setup
```bash
  git clone https://github.com/chewton2k/embd-ide.git
  cd embd-ide
  npm install
```

#### Run - Development
```bash
  npm run tauri:dev
```
Builds and launches a live development version of the app with hot-reloading.

#### Build - Production App
```bash
npm run tauri:build
```
Compiles and installs the app to your system's application folder.


## Other important information:
### Beta Testing

To test the latest in-progress features, switch to the beta branch after cloning:

> **Warning:** Beta builds are unstable. Do not use them for important work.

### Themes
Themes are configured in `src/lib/stores.ts` inside the `THEMES` export:
```ts
export const THEMES: ThemePreset[] = [ ... ]
```

To create a custom theme, see [`docs/Themes.md`](docs/Themes.md).

---

### Feedback
Have a bug report or suggestion? [Fill out the feedback form here](https://docs.google.com/forms/d/e/1FAIpQLSe1Dsog4TyfOHtNnQaMMKLqfcnWlTFNW2U9RcAnF-E5PB_NCw/viewform?usp=publish-editor)
