//! Finding the `claude` binary.
//!
//! It is routinely not on `PATH`: the VS Code extension ships its own copy
//! and installs none.

use std::path::PathBuf;

/// Locates the `claude` executable, most explicit source first.
pub fn claude_binary() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("DEVPLANE_CLAUDE_BIN") {
        let p = PathBuf::from(p);
        if p.is_file() {
            return Some(p);
        }
    }
    if let Some(p) = on_path("claude") {
        return Some(p);
    }
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        // The native installer.
        let local = home.join(".claude/local/claude");
        if local.is_file() {
            return Some(local);
        }
        // The VS Code extension's bundled copies: newest by name (versions
        // sort lexically within a major).
        if let Some(p) = newest_extension_binary(&home.join(".vscode/extensions")) {
            return Some(p);
        }
        if let Some(p) = newest_extension_binary(&home.join(".vscode-insiders/extensions")) {
            return Some(p);
        }
        if let Some(p) = newest_extension_binary(&home.join(".cursor/extensions")) {
            return Some(p);
        }
    }
    None
}

fn newest_extension_binary(dir: &std::path::Path) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("anthropic.claude-code-"))
                .unwrap_or(false)
        })
        .collect();
    candidates.sort();
    candidates
        .into_iter()
        .rev()
        .map(|p| p.join("resources/native-binary/claude"))
        .find(|p| p.is_file())
}

fn on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_explicit_override_wins_when_it_exists() {
        // A stale override falls through to discovery.
        unsafe { std::env::set_var("DEVPLANE_CLAUDE_BIN", "/definitely/not/here") };
        let found = claude_binary();
        unsafe { std::env::remove_var("DEVPLANE_CLAUDE_BIN") };
        assert!(found != Some(PathBuf::from("/definitely/not/here")));
    }

    #[test]
    fn extension_candidates_sort_newest_first() {
        let dir = std::env::temp_dir().join(format!("vp-ext-{}", std::process::id()));
        for v in [
            "anthropic.claude-code-2.1.266-darwin-arm64",
            "anthropic.claude-code-2.1.270-darwin-arm64",
        ] {
            let p = dir.join(v).join("resources/native-binary");
            std::fs::create_dir_all(&p).unwrap();
            std::fs::write(p.join("claude"), b"#!/bin/sh\n").unwrap();
        }
        let found = newest_extension_binary(&dir).unwrap();
        assert!(found.to_string_lossy().contains("2.1.270"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
