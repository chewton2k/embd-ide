use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{self, Command};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let task = args.first().map(|s| s.as_str()).unwrap_or("help");

    match task {
        "bundle" => bundle(),
        "install" => {
            bundle();
            install();
        }
        "clean" => clean(),
        "help" | "--help" | "-h" => print_help(),
        other => {
            eprintln!("Unknown task: {other}");
            print_help();
            process::exit(1);
        }
    }
}

fn print_help() {
    println!(
        "\
Usage: cargo xtask <task>

Tasks:
  bundle    Build a release bundle for the current platform
  install   Build and install to the system
  clean     Remove build artifacts
  help      Show this help message"
    );
}

/// Root of the workspace (parent of crates/)
fn workspace_root() -> PathBuf {
    let manifest = env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into());
    Path::new(&manifest)
        .ancestors()
        .nth(2)
        .unwrap_or(Path::new("."))
        .to_path_buf()
}

/// Read version from embd-app's Cargo.toml
fn app_version(root: &Path) -> String {
    let cargo_toml = root.join("crates/embd-app/Cargo.toml");
    let content = fs::read_to_string(&cargo_toml).expect("Failed to read embd-app/Cargo.toml");
    for line in content.lines() {
        if line.starts_with("version") {
            if let Some(v) = line.split('"').nth(1) {
                return v.to_string();
            }
        }
    }
    "0.0.0".to_string()
}

fn bundle() {
    let root = workspace_root();
    let version = app_version(&root);

    println!("Building embd v{version} (release)...");

    let status = Command::new("cargo")
        .args(["build", "--release", "--package", "embd-app"])
        .current_dir(&root)
        .status()
        .expect("Failed to run cargo build");

    if !status.success() {
        eprintln!("Build failed");
        process::exit(1);
    }

    if cfg!(target_os = "macos") {
        bundle_macos(&root, &version);
    } else if cfg!(target_os = "linux") {
        bundle_linux(&root, &version);
    } else if cfg!(target_os = "windows") {
        bundle_windows(&root, &version);
    } else {
        eprintln!("Unsupported platform for bundling");
        process::exit(1);
    }
}

// ── macOS .app bundle ────────────────────────────────────────────────

fn bundle_macos(root: &Path, version: &str) {
    let binary = root.join("target/release/embd");
    if !binary.exists() {
        eprintln!("Binary not found at {}", binary.display());
        process::exit(1);
    }

    let app_dir = root.join("target/embd.app");
    let macos_dir = app_dir.join("Contents/MacOS");
    let resources_dir = app_dir.join("Contents/Resources");

    if app_dir.exists() {
        fs::remove_dir_all(&app_dir).expect("Failed to remove old .app");
    }

    fs::create_dir_all(&macos_dir).expect("Failed to create MacOS dir");
    fs::create_dir_all(&resources_dir).expect("Failed to create Resources dir");

    fs::copy(&binary, macos_dir.join("embd")).expect("Failed to copy binary");

    let icon = root.join("assets/embd.icns");
    if icon.exists() {
        fs::copy(&icon, resources_dir.join("AppIcon.icns")).expect("Failed to copy icon");
    }

    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>embd</string>
    <key>CFBundleDisplayName</key>
    <string>embd</string>
    <key>CFBundleIdentifier</key>
    <string>com.embd.ide</string>
    <key>CFBundleVersion</key>
    <string>{version}</string>
    <key>CFBundleShortVersionString</key>
    <string>{version}</string>
    <key>CFBundleExecutable</key>
    <string>embd</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>LSMinimumSystemVersion</key>
    <string>13.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSSupportsAutomaticGraphicsSwitching</key>
    <true/>
</dict>
</plist>
"#
    );

    fs::write(app_dir.join("Contents/Info.plist"), plist).expect("Failed to write Info.plist");

    println!("Built: {}", app_dir.display());
    println!("Version: {version}");
}

// ── Linux bundle ─────────────────────────────────────────────────────

fn bundle_linux(root: &Path, version: &str) {
    let binary = root.join("target/release/embd");
    if !binary.exists() {
        eprintln!("Binary not found at {}", binary.display());
        process::exit(1);
    }

    let bundle_dir = root.join("target/embd-linux");

    if bundle_dir.exists() {
        fs::remove_dir_all(&bundle_dir).expect("Failed to remove old bundle");
    }

    let bin_dir = bundle_dir.join("usr/bin");
    let desktop_dir = bundle_dir.join("usr/share/applications");
    let icon_dir = bundle_dir.join("usr/share/icons/hicolor/256x256/apps");

    fs::create_dir_all(&bin_dir).expect("Failed to create bin dir");
    fs::create_dir_all(&desktop_dir).expect("Failed to create desktop dir");
    fs::create_dir_all(&icon_dir).expect("Failed to create icon dir");

    fs::copy(&binary, bin_dir.join("embd")).expect("Failed to copy binary");

    // Copy icon if available
    let icon = root.join("public/embd_logo.png");
    if icon.exists() {
        fs::copy(&icon, icon_dir.join("embd.png")).expect("Failed to copy icon");
    }

    // Desktop entry
    let desktop = format!(
        "\
[Desktop Entry]
Name=embd
Comment=Lightweight GPU-accelerated code editor
Exec=embd
Icon=embd
Type=Application
Categories=Development;TextEditor;IDE;
Keywords=editor;code;ide;rust;
StartupWMClass=embd
Version={version}
"
    );
    fs::write(desktop_dir.join("embd.desktop"), desktop).expect("Failed to write .desktop");

    println!("Built: {}", bundle_dir.display());
    println!("Version: {version}");
    println!();
    println!("To install:");
    println!("  sudo cp target/embd-linux/usr/bin/embd /usr/local/bin/");
    println!("  sudo cp target/embd-linux/usr/share/applications/embd.desktop /usr/share/applications/");
    println!("  sudo cp target/embd-linux/usr/share/icons/hicolor/256x256/apps/embd.png /usr/share/icons/hicolor/256x256/apps/");
}

// ── Windows bundle ───────────────────────────────────────────────────

fn bundle_windows(root: &Path, version: &str) {
    let binary = root.join("target/release/embd.exe");
    if !binary.exists() {
        eprintln!("Binary not found at {}", binary.display());
        process::exit(1);
    }

    let bundle_dir = root.join("target/embd-windows");

    if bundle_dir.exists() {
        fs::remove_dir_all(&bundle_dir).expect("Failed to remove old bundle");
    }

    fs::create_dir_all(&bundle_dir).expect("Failed to create bundle dir");

    fs::copy(&binary, bundle_dir.join("embd.exe")).expect("Failed to copy binary");

    // Copy icon if available
    let icon = root.join("public/embd_logo.png");
    if icon.exists() {
        fs::copy(&icon, bundle_dir.join("embd.png")).expect("Failed to copy icon");
    }

    println!("Built: {}", bundle_dir.display());
    println!("Version: {version}");
    println!();
    println!("To install, add {} to your PATH or move embd.exe to a directory in your PATH.", bundle_dir.display());
}

// ── Install ──────────────────────────────────────────────────────────

fn install() {
    let root = workspace_root();

    if cfg!(target_os = "macos") {
        let app_dir = root.join("target/embd.app");
        let dest = Path::new("/Applications/embd.app");

        if !app_dir.exists() {
            eprintln!("No .app bundle found. Run `cargo xtask bundle` first.");
            process::exit(1);
        }

        if dest.exists() {
            fs::remove_dir_all(dest).expect("Failed to remove old /Applications/embd.app");
        }

        let status = Command::new("cp")
            .args(["-r", &app_dir.to_string_lossy(), &dest.to_string_lossy()])
            .status()
            .expect("Failed to copy to /Applications");

        if !status.success() {
            eprintln!("Failed to install to /Applications");
            process::exit(1);
        }

        println!("Installed to /Applications/embd.app");
    } else if cfg!(target_os = "linux") {
        let bundle_dir = root.join("target/embd-linux");
        if !bundle_dir.exists() {
            eprintln!("No bundle found. Run `cargo xtask bundle` first.");
            process::exit(1);
        }

        let copies = [
            ("usr/bin/embd", "/usr/local/bin/embd"),
            ("usr/share/applications/embd.desktop", "/usr/share/applications/embd.desktop"),
            ("usr/share/icons/hicolor/256x256/apps/embd.png", "/usr/share/icons/hicolor/256x256/apps/embd.png"),
        ];

        for (src, dest) in &copies {
            let src_path = bundle_dir.join(src);
            if src_path.exists() {
                let dest_path = Path::new(dest);
                if let Some(parent) = dest_path.parent() {
                    let _ = Command::new("sudo")
                        .args(["mkdir", "-p", &parent.to_string_lossy()])
                        .status();
                }
                let status = Command::new("sudo")
                    .args(["cp", &src_path.to_string_lossy(), dest])
                    .status()
                    .expect("Failed to run sudo cp");
                if !status.success() {
                    eprintln!("Failed to install {dest}");
                    process::exit(1);
                }
            }
        }

        println!("Installed embd to /usr/local/bin/embd");
    } else if cfg!(target_os = "windows") {
        let bundle_dir = root.join("target/embd-windows");
        if !bundle_dir.exists() {
            eprintln!("No bundle found. Run `cargo xtask bundle` first.");
            process::exit(1);
        }

        // Install to %LOCALAPPDATA%\embd
        let local_app_data = env::var("LOCALAPPDATA").unwrap_or_else(|_| {
            eprintln!("LOCALAPPDATA not set");
            process::exit(1);
        });
        let install_dir = PathBuf::from(&local_app_data).join("embd");
        fs::create_dir_all(&install_dir).expect("Failed to create install dir");

        let exe_src = bundle_dir.join("embd.exe");
        let exe_dest = install_dir.join("embd.exe");
        fs::copy(&exe_src, &exe_dest).expect("Failed to copy embd.exe");

        println!("Installed to {}", exe_dest.display());
        println!("Add {} to your PATH to run `embd` from anywhere.", install_dir.display());
    }
}

// ── Clean ────────────────────────────────────────────────────────────

fn clean() {
    let root = workspace_root();

    for name in &["embd.app", "embd-linux", "embd-windows"] {
        let dir = root.join("target").join(name);
        if dir.exists() {
            fs::remove_dir_all(&dir).expect(&format!("Failed to remove {}", dir.display()));
            println!("Removed {}", dir.display());
        }
    }

    let status = Command::new("cargo")
        .args(["clean"])
        .current_dir(&root)
        .status()
        .expect("Failed to run cargo clean");

    if status.success() {
        println!("Cleaned build artifacts");
    }
}
