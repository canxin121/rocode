use crate::Config;
use anyhow::{Context, Result};
use jsonc_parser::{parse_to_serde_value, ParseOptions};
use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn get_global_config_paths() -> Vec<PathBuf> {
    let config_dir = if cfg!(target_os = "macos") {
        dirs::config_dir().unwrap_or_else(|| PathBuf::from("~/.config"))
    } else if cfg!(target_os = "windows") {
        dirs::config_dir().unwrap_or_else(|| PathBuf::from("%APPDATA%"))
    } else {
        dirs::config_dir().unwrap_or_else(|| PathBuf::from("~/.config"))
    };

    vec![config_dir.join("rocode/rocode")]
}

/// Substitute `{env:VAR}` patterns with environment variable values.
/// Works on the raw JSONC text before parsing.
pub(super) fn substitute_env_vars(text: &str) -> String {
    let re = regex::Regex::new(r"\{env:([^}]+)\}").unwrap();
    re.replace_all(text, |caps: &regex::Captures| {
        let var_name = &caps[1];
        std::env::var(var_name).unwrap_or_default()
    })
    .to_string()
}

/// Resolve `{file:path}` patterns by reading file contents.
/// Skips patterns on commented lines. Resolves relative paths from `base_dir`.
pub(super) fn resolve_file_references(text: &str, base_dir: &Path) -> Result<String> {
    let re = regex::Regex::new(r"\{file:([^}]+)\}").unwrap();

    let mut result = text.to_string();

    // Collect all matches first to avoid borrow issues
    let matches: Vec<(String, String)> = re
        .captures_iter(text)
        .map(|caps| {
            let full_match = caps.get(0).unwrap().as_str().to_string();
            let file_path_str = caps[1].to_string();
            (full_match, file_path_str)
        })
        .collect();

    for (full_match, file_path_str) in matches {
        // Check if the match is on a commented line
        let match_start = match text.find(&full_match) {
            Some(pos) => pos,
            None => continue,
        };
        let line_start = text[..match_start].rfind('\n').map(|p| p + 1).unwrap_or(0);
        let line_end = text[line_start..]
            .find('\n')
            .map(|p| line_start + p)
            .unwrap_or(text.len());
        let line = &text[line_start..line_end];
        if line.trim().starts_with("//") {
            continue;
        }

        // Resolve the file path
        let resolved = if let Some(stripped) = file_path_str.strip_prefix("~/") {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("~"))
                .join(stripped)
        } else if Path::new(&file_path_str).is_absolute() {
            PathBuf::from(&file_path_str)
        } else {
            base_dir.join(&file_path_str)
        };

        // Read the file
        let content = fs::read_to_string(&resolved).with_context(|| {
            format!(
                "bad file reference: \"{}\" - {} does not exist",
                full_match,
                resolved.display()
            )
        })?;
        let content = content.trim();

        // Escape for JSON string context (newlines, quotes)
        let escaped = content
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t");

        result = result.replace(&full_match, &escaped);
    }

    Ok(result)
}

pub(super) fn parse_jsonc(content: &str) -> Result<Config> {
    let parse_options = ParseOptions {
        allow_trailing_commas: true,
        ..Default::default()
    };
    let parsed = parse_to_serde_value(content, &parse_options)
        .with_context(|| "Failed to parse JSONC")?
        .context("Config content is empty")?;
    serde_json::from_value(parsed).with_context(|| "Failed to parse config JSON")
}
