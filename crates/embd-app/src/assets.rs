use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::LazyLock;

use gpui::{AssetSource, Hsla, Result, SharedString, rgb};

// ── Embedded SVG icons (Material Icon Theme) ────────────────────────

macro_rules! icon {
    ($m:expr, $name:literal) => {
        $m.insert(
            concat!("icons/", $name, ".svg"),
            include_bytes!(concat!("../../../assets/icons/", $name, ".svg")).as_slice(),
        );
    };
}

static ICONS: LazyLock<HashMap<&'static str, &'static [u8]>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    // Generic
    icon!(m, "file");
    icon!(m, "folder");
    icon!(m, "folder-open");
    icon!(m, "document");
    icon!(m, "settings");
    icon!(m, "console");
    icon!(m, "image");
    icon!(m, "database");
    icon!(m, "lock");
    icon!(m, "git");
    // Languages
    icon!(m, "rust");
    icon!(m, "javascript");
    icon!(m, "typescript");
    icon!(m, "react");
    icon!(m, "react_ts");
    icon!(m, "python");
    icon!(m, "go");
    icon!(m, "c");
    icon!(m, "cpp");
    icon!(m, "java");
    icon!(m, "kotlin");
    icon!(m, "scala");
    icon!(m, "swift");
    icon!(m, "dart");
    icon!(m, "ruby");
    icon!(m, "lua");
    icon!(m, "zig");
    icon!(m, "csharp");
    icon!(m, "php");
    icon!(m, "r");
    icon!(m, "haskell");
    icon!(m, "ocaml");
    icon!(m, "elixir");
    icon!(m, "erlang");
    icon!(m, "perl");
    icon!(m, "clojure");
    icon!(m, "scheme");
    icon!(m, "prolog");
    icon!(m, "tex");
    // Web
    icon!(m, "html");
    icon!(m, "css");
    icon!(m, "xml");
    icon!(m, "graphql");
    icon!(m, "sql");
    icon!(m, "vue");
    // Data / config
    icon!(m, "json");
    icon!(m, "toml");
    icon!(m, "yaml");
    icon!(m, "markdown");
    icon!(m, "docker");
    icon!(m, "cargo");
    icon!(m, "npm");
    // Docs / media
    icon!(m, "pdf");
    icon!(m, "readme");
    icon!(m, "svg-file");
    icon!(m, "video");
    icon!(m, "audio");
    icon!(m, "font");
    icon!(m, "archive");
    icon!(m, "certificate");
    m
});

pub struct EmbeddedAssets;

impl AssetSource for EmbeddedAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(ICONS.get(path).map(|data| Cow::Borrowed(*data)))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(ICONS
            .keys()
            .filter(|k| k.starts_with(path))
            .map(|k| SharedString::from(*k))
            .collect())
    }
}

// ── File extension → icon path mapping ──────────────────────────────

pub fn icon_for_path(path: &str, is_dir: bool, is_expanded: bool) -> &'static str {
    if is_dir {
        return if is_expanded {
            "icons/folder-open.svg"
        } else {
            "icons/folder.svg"
        };
    }

    // Check special filenames first
    let name = path.rsplit('/').next().unwrap_or(path);
    match name {
        "Dockerfile" | "Containerfile" | "docker-compose.yml"
        | "docker-compose.yaml" => return "icons/docker.svg",
        "Makefile" | "makefile" | "CMakeLists.txt" => return "icons/settings.svg",
        ".gitignore" | ".gitmodules" | ".gitattributes" => return "icons/git.svg",
        "Cargo.toml" => return "icons/cargo.svg",
        "Cargo.lock" => return "icons/cargo.svg",
        "package.json" | "package-lock.json" => return "icons/npm.svg",
        "yarn.lock" | "pnpm-lock.yaml" | "Gemfile.lock" => return "icons/lock.svg",
        "README" | "README.md" | "README.txt" | "README.rst"
        | "readme" | "readme.md" | "readme.txt" => return "icons/readme.svg",
        "LICENSE" | "LICENSE.md" | "LICENSE.txt" | "LICENCE"
        | "license" | "licence" => return "icons/certificate.svg",
        _ => {}
    }

    let ext = path.rsplit('.').next().unwrap_or("");
    match ext {
        // Rust
        "rs" => "icons/rust.svg",
        // JavaScript
        "js" | "mjs" | "cjs" => "icons/javascript.svg",
        "jsx" => "icons/react.svg",
        // TypeScript
        "ts" | "mts" | "cts" => "icons/typescript.svg",
        "tsx" => "icons/react_ts.svg",
        // Python
        "py" | "pyi" | "pyw" => "icons/python.svg",
        // Go
        "go" => "icons/go.svg",
        // C / C++
        "c" | "h" => "icons/c.svg",
        "cpp" | "cc" | "cxx" | "hpp" | "hxx" => "icons/cpp.svg",
        // C#
        "cs" => "icons/csharp.svg",
        // JVM
        "java" => "icons/java.svg",
        "kt" | "kts" => "icons/kotlin.svg",
        "scala" | "sc" => "icons/scala.svg",
        "clj" | "cljs" | "cljc" | "edn" => "icons/clojure.svg",
        // Apple / mobile
        "swift" => "icons/swift.svg",
        "dart" => "icons/dart.svg",
        // Scripting
        "rb" | "erb" => "icons/ruby.svg",
        "lua" => "icons/lua.svg",
        "pl" | "pm" => "icons/perl.svg",
        "php" | "phtml" => "icons/php.svg",
        "r" | "R" | "rmd" | "Rmd" => "icons/r.svg",
        // Functional
        "hs" | "lhs" => "icons/haskell.svg",
        "ml" | "mli" => "icons/ocaml.svg",
        "ex" | "exs" => "icons/elixir.svg",
        "erl" | "hrl" => "icons/erlang.svg",
        "scm" | "ss" | "rkt" => "icons/scheme.svg",
        "pro" | "pl_prolog" => "icons/prolog.svg",
        // Systems
        "zig" => "icons/zig.svg",
        // TeX / LaTeX
        "tex" | "latex" | "sty" | "cls" | "bib" => "icons/tex.svg",
        // Web
        "html" | "htm" => "icons/html.svg",
        "css" | "scss" | "sass" | "less" => "icons/css.svg",
        "vue" => "icons/vue.svg",
        "xml" | "xsl" | "xslt" => "icons/xml.svg",
        "svg" => "icons/svg-file.svg",
        "graphql" | "gql" => "icons/graphql.svg",
        // Data / config
        "json" | "jsonc" | "json5" => "icons/json.svg",
        "toml" => "icons/toml.svg",
        "yaml" | "yml" => "icons/yaml.svg",
        "sql" => "icons/sql.svg",
        "ini" | "env" | "cfg" | "conf" => "icons/settings.svg",
        // Docs
        "pdf" => "icons/pdf.svg",
        "md" | "markdown" | "mdx" => "icons/markdown.svg",
        "txt" | "log" | "csv" | "tsv" | "rst" => "icons/document.svg",
        // Shell
        "sh" | "bash" | "zsh" | "fish" | "ps1" => "icons/console.svg",
        // Images
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "ico" | "bmp" | "tiff" => "icons/image.svg",
        // Video
        "mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "webm" => "icons/video.svg",
        // Audio
        "mp3" | "wav" | "flac" | "ogg" | "aac" | "m4a" | "wma" => "icons/audio.svg",
        // Fonts
        "ttf" | "otf" | "woff" | "woff2" | "eot" => "icons/font.svg",
        // Archives
        "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" | "tgz" => "icons/archive.svg",
        // Certificates
        "pem" | "crt" | "cer" | "key" | "p12" | "pfx" => "icons/certificate.svg",
        // Lock files
        "lock" => "icons/lock.svg",
        // Default
        _ => "icons/file.svg",
    }
}

/// Returns the Material theme color for a given icon path.
pub fn icon_color(icon_path: &str) -> Hsla {
    match icon_path {
        // Languages — match Material Icon Theme colors
        "icons/rust.svg" | "icons/cargo.svg" => rgb(0xff7043).into(),
        "icons/javascript.svg" => rgb(0xffca28).into(),
        "icons/typescript.svg" => rgb(0x0288d1).into(),
        "icons/react.svg" | "icons/react_ts.svg" => rgb(0x00bcd4).into(),
        "icons/python.svg" => rgb(0x0288d1).into(),
        "icons/go.svg" => rgb(0x00acc1).into(),
        "icons/c.svg" => rgb(0x0288d1).into(),
        "icons/cpp.svg" => rgb(0x0288d1).into(),
        "icons/csharp.svg" => rgb(0x4caf50).into(),
        "icons/java.svg" => rgb(0xf44336).into(),
        "icons/kotlin.svg" => rgb(0x7e57c2).into(),
        "icons/scala.svg" => rgb(0xf44336).into(),
        "icons/clojure.svg" => rgb(0x4caf50).into(),
        "icons/swift.svg" => rgb(0xff6d00).into(),
        "icons/dart.svg" => rgb(0x00bcd4).into(),
        "icons/ruby.svg" => rgb(0xf44336).into(),
        "icons/lua.svg" => rgb(0x0288d1).into(),
        "icons/perl.svg" => rgb(0x0288d1).into(),
        "icons/php.svg" => rgb(0x6a1b9a).into(),
        "icons/r.svg" => rgb(0x2196f3).into(),
        "icons/haskell.svg" => rgb(0x7e57c2).into(),
        "icons/ocaml.svg" => rgb(0xef6c00).into(),
        "icons/elixir.svg" => rgb(0x7e57c2).into(),
        "icons/erlang.svg" => rgb(0xf44336).into(),
        "icons/scheme.svg" => rgb(0x7e57c2).into(),
        "icons/prolog.svg" => rgb(0xff9800).into(),
        "icons/zig.svg" => rgb(0xf6a623).into(),
        "icons/tex.svg" => rgb(0x4caf50).into(),
        // Web
        "icons/html.svg" => rgb(0xe65100).into(),
        "icons/css.svg" => rgb(0x7e57c2).into(),
        "icons/vue.svg" => rgb(0x4caf50).into(),
        "icons/xml.svg" => rgb(0xff6d00).into(),
        "icons/svg-file.svg" => rgb(0xff6d00).into(),
        "icons/graphql.svg" => rgb(0xe535ab).into(),
        // Data / config
        "icons/json.svg" => rgb(0xfdd835).into(),
        "icons/toml.svg" => rgb(0x9e9e9e).into(),
        "icons/yaml.svg" => rgb(0x9e9e9e).into(),
        "icons/sql.svg" | "icons/database.svg" => rgb(0x00bcd4).into(),
        "icons/settings.svg" => rgb(0x42a5f5).into(),
        "icons/docker.svg" => rgb(0x0288d1).into(),
        "icons/npm.svg" => rgb(0xcb3837).into(),
        // Docs
        "icons/pdf.svg" => rgb(0xf44336).into(),
        "icons/readme.svg" => rgb(0x42a5f5).into(),
        "icons/markdown.svg" => rgb(0x42a5f5).into(),
        "icons/document.svg" => rgb(0x90a4ae).into(),
        // Shell
        "icons/console.svg" => rgb(0x4caf50).into(),
        // Media
        "icons/image.svg" => rgb(0x26a69a).into(),
        "icons/video.svg" => rgb(0x7e57c2).into(),
        "icons/audio.svg" => rgb(0x26a69a).into(),
        // Fonts
        "icons/font.svg" => rgb(0xf44336).into(),
        // Archives
        "icons/archive.svg" => rgb(0xff9800).into(),
        // Certificates
        "icons/certificate.svg" => rgb(0x4caf50).into(),
        // Git
        "icons/git.svg" => rgb(0xf44336).into(),
        // Lock
        "icons/lock.svg" => rgb(0x9e9e9e).into(),
        // Folders
        "icons/folder.svg" | "icons/folder-open.svg" => rgb(0x90a4ae).into(),
        // Default
        _ => rgb(0x90a4ae).into(),
    }
}
