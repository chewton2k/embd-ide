use embd_core::Result;
use ignore::WalkBuilder;
use std::path::Path;

/// Result of a file search across the project.
#[derive(Debug, Clone)]
pub struct FileMatch {
    /// Relative path from the project root.
    pub relative_path: String,
    /// Match score (higher = better match).
    pub score: i32,
}

/// Search for files matching a query string within a project root.
/// Uses the `ignore` crate to respect .gitignore rules.
///
/// This replaces the frontend's JS-based file search with a fast
/// Rust implementation that handles gitignore natively.
pub fn find_files(root: &Path, query: &str, max_results: usize) -> Result<Vec<FileMatch>> {
    let query_lower = query.to_lowercase();
    let query_parts: Vec<&str> = query_lower.split_whitespace().collect();

    let mut matches: Vec<FileMatch> = Vec::new();

    let walker = WalkBuilder::new(root)
        .hidden(false) // don't skip hidden files (we want .env, etc.)
        .git_ignore(true) // respect .gitignore
        .git_global(true)
        .git_exclude(true)
        .filter_entry(|entry| {
            // Skip .git directory itself
            let name = entry.file_name().to_string_lossy();
            name != ".git"
        })
        .build();

    for entry in walker.flatten() {
        if !entry.file_type().map_or(false, |ft| ft.is_file()) {
            continue;
        }

        let rel = match entry.path().strip_prefix(root) {
            Ok(r) => r.to_string_lossy().to_string(),
            Err(_) => continue,
        };

        let rel_lower = rel.to_lowercase();
        let filename_lower = entry
            .file_name()
            .to_string_lossy()
            .to_lowercase();

        // Score the match
        let score = if query_parts.is_empty() {
            0 // No query = list all, no scoring needed
        } else {
            let mut s: i32 = 0;
            let mut all_match = true;

            for part in &query_parts {
                if filename_lower.contains(part) {
                    s += 20; // Filename match is more valuable
                } else if rel_lower.contains(part) {
                    s += 5; // Path match
                } else {
                    all_match = false;
                    break;
                }
            }

            if !all_match {
                continue; // Skip non-matching files
            }

            // Bonus for shorter paths (penalize deep nesting)
            s -= (rel.len() / 10) as i32;
            // Bonus for exact filename match
            if filename_lower == query_lower {
                s += 100;
            }
            s
        };

        matches.push(FileMatch {
            relative_path: rel,
            score,
        });

        // Early exit if we've collected way more than needed
        if matches.len() > max_results * 10 {
            break;
        }
    }

    // Sort by score descending, then by path length ascending
    matches.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then(a.relative_path.len().cmp(&b.relative_path.len()))
    });

    matches.truncate(max_results);
    Ok(matches)
}
