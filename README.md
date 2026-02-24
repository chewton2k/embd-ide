# embd-IDE

- [App Setup](#setup)
  - [Running Development](#run-development)
  - [Downloading App](#build-production)
- [Beta Testing Info](#beta-testing)
- [Feedback Form](#feedback-form)

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

#### Run - Development
```bash
  npm run tauri:dev
```
Builds and spawns a temporary version of the app.

#### Build - Production App
```bash
npm run tauri:build
```
Builds the app and moves it to your application folder.

### Beta Testing

If you would like to test on beta, clone the repository then switch over to the open beta branch.


### [Feedback Form](https://docs.google.com/forms/d/e/1FAIpQLSe1Dsog4TyfOHtNnQaMMKLqfcnWlTFNW2U9RcAnF-E5PB_NCw/viewform?usp=publish-editor)
