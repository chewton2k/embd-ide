# embd-IDE

## How to get started: 

### Prerequisites: 
1. Node.js (v18+)
2. Rust
```bash
brew install rust
```
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

#### Run (Development)
```bash
  npm run tauri:dev
```

#### Build (Production)
```bash
npm run tauri:build
```
Builds the app and moves it to your application folder.


### [Feedback Form](https://docs.google.com/forms/d/e/1FAIpQLSe1Dsog4TyfOHtNnQaMMKLqfcnWlTFNW2U9RcAnF-E5PB_NCw/viewform?usp=publish-editor)
