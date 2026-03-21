pub mod json {
    use rocode_core::contracts::patch::keys as patch_keys;
    use rocode_core::contracts::tools::{arg_keys as tool_arg_keys, BuiltinToolName};
    use serde::Serialize;

    fn parse_json_object(input: &str) -> Option<serde_json::Value> {
        serde_json::from_str::<serde_json::Value>(input)
            .ok()
            .filter(serde_json::Value::is_object)
    }

    fn parse_json_object_with_recovery(input: &str) -> Option<serde_json::Value> {
        let cleaned = input.trim().trim_start_matches('\u{feff}').trim();
        parse_json_object(cleaned)
    }

    /// Try to parse `input` as a JSON object with extra recovery steps:
    /// - trims surrounding whitespace and BOM
    /// - re-escapes literal control characters in string values
    /// - unwraps one layer when `input` itself is a JSON string containing JSON
    ///
    /// Returns `Some(Value::Object)` on success, `None` otherwise.
    pub fn try_parse_json_object_robust(input: &str) -> Option<serde_json::Value> {
        if let Some(val) = parse_json_object_with_recovery(input) {
            return Some(val);
        }
        if let Ok(inner) = serde_json::from_str::<String>(input) {
            if let Some(val) = parse_json_object_with_recovery(&inner) {
                return Some(val);
            }
        }
        None
    }

    /// Backward-compatible helper retained for existing call sites.
    pub fn try_parse_json_object(input: &str) -> Option<serde_json::Value> {
        try_parse_json_object_robust(input)
    }

    /// Best-effort recovery for truncated/malformed JSON-ish tool argument strings.
    /// Returns an object only when required fields for the given tool can be extracted.
    pub fn recover_tool_arguments_from_jsonish(
        tool_name: &str,
        input: &str,
    ) -> Option<serde_json::Value> {
        let _tool = BuiltinToolName::parse(tool_name)?;
        try_parse_json_object_robust(input)
    }

    // -----------------------------------------------------------------------
    // Ultra tool-call recovery: "Never repair JSON. Recover structure."
    // -----------------------------------------------------------------------

    /// Top-level entry point for structural recovery of malformed tool-call
    /// arguments.  Works through six stages, from cheapest to most aggressive.
    pub fn recover_tool_call_ultra(tool: &str, raw: &str) -> Option<serde_json::Value> {
        let _tool = BuiltinToolName::parse(tool)?;
        try_parse_json_object_robust(raw)
    }

    // -- Stage 1 helpers ----------------------------------------------------

    fn ultra_clean_raw(raw: &str) -> String {
        // Delegate to the unified sanitize phase (Phase 0 only) which handles
        // BOM, ANSI escapes, XML/HTML wrappers, markdown fences, and trailing
        // semicolons — without modifying JSON structure.
        let (sanitized, _) = crate::jsonish_parse::sanitize_standalone(raw);
        sanitized
    }

    // -- Stage 2 helpers ----------------------------------------------------

    /// Extract multiple `{…}` candidate regions and pick the one most likely
    /// to be the actual tool-call JSON (scored by presence of expected keys
    /// and parsability).
    fn ultra_pick_best_candidate(input: &str) -> Option<String> {
        let last_brace = input.rfind('}')?;
        let candidates = ultra_extract_candidates(input, last_brace);
        candidates
            .into_iter()
            .max_by_key(|c| ultra_score_candidate(c))
            .map(|s| s.to_string())
    }

    /// Generate candidate regions by pairing each `{` with the last `}`.
    fn ultra_extract_candidates(input: &str, last_brace: usize) -> Vec<&str> {
        let mut res = Vec::new();
        let mut pos = 0;
        while let Some(offset) = input[pos..].find('{') {
            let start = pos + offset;
            if start < last_brace {
                res.push(&input[start..=last_brace]);
            }
            pos = start + 1;
            // Limit candidates to avoid quadratic behaviour on huge inputs.
            if res.len() >= 16 {
                break;
            }
        }
        // Also try the truncated tail (no closing brace) for stream-cut JSON.
        if res.is_empty() {
            if let Some(offset) = input.find('{') {
                res.push(&input[offset..]);
            }
        }
        res
    }

    /// Score a candidate region: higher = more likely to be the real tool call.
    fn ultra_score_candidate(s: &str) -> i32 {
        let mut score: i32 = 0;
        // Presence of known tool-call keys (quoted to avoid HTML attribute matches).
        if s.contains("\"file_path\"") || s.contains("\"filePath\"") {
            score += 100;
        }
        if s.contains("\"content\"") {
            score += 80;
        }
        if s.contains("\"command\"") || s.contains("\"old_string\"") || s.contains("\"new_string\"")
        {
            score += 60;
        }
        // Shorter candidates are preferred (less garbage included).
        score -= (s.len() / 1000) as i32;
        // Bonus if it actually parses.
        if serde_json::from_str::<serde_json::Value>(s).is_ok() {
            score += 200;
        }
        score
    }

    // -- Stage 4 helpers ----------------------------------------------------

    fn ultra_repair_truncated(input: &str) -> Option<serde_json::Value> {
        // Use the full repair pipeline in aggressive mode — handles unclosed
        // strings, missing commas/colons, bracket balancing, escape repair, etc.
        let (repaired, _) = crate::jsonish_parse::repair_json_standalone(input, true);
        if let Ok(v @ serde_json::Value::Object(_)) =
            serde_json::from_str::<serde_json::Value>(&repaired)
        {
            return Some(v);
        }

        // Fallback: naive quote + brace balancing for edge cases the pipeline misses
        let mut s = input.to_string();
        if !ultra_count_unescaped_quotes(&s).is_multiple_of(2) {
            s.push('"');
        }
        let open = s.chars().filter(|&c| c == '{').count();
        let close = s.chars().filter(|&c| c == '}').count();
        for _ in 0..open.saturating_sub(close) {
            s.push('}');
        }
        if let Ok(v @ serde_json::Value::Object(_)) = serde_json::from_str::<serde_json::Value>(&s)
        {
            return Some(v);
        }
        None
    }

    /// Count `"` that are NOT preceded by an odd run of backslashes.
    fn ultra_count_unescaped_quotes(s: &str) -> usize {
        let mut count = 0;
        let bytes = s.as_bytes();
        for i in 0..bytes.len() {
            if bytes[i] == b'"' {
                let mut backslashes = 0;
                let mut j = i;
                while j > 0 && bytes[j - 1] == b'\\' {
                    backslashes += 1;
                    j -= 1;
                }
                if backslashes % 2 == 0 {
                    count += 1;
                }
            }
        }
        count
    }

    // -- Stage 5 helpers ----------------------------------------------------

    fn ultra_recover_loose_object(input: &str) -> Option<serde_json::Value> {
        let mut map = serde_json::Map::new();
        let mut pos = 0;
        while let Some((k, v, next)) = ultra_scan_field(input, pos) {
            map.insert(k, serde_json::Value::String(v));
            pos = next;
        }
        if map.is_empty() {
            return None;
        }
        Some(serde_json::Value::Object(map))
    }

    /// Scan for the next `"key": "value"` pair starting at `start`.
    fn ultra_scan_field(input: &str, start: usize) -> Option<(String, String, usize)> {
        let rest = &input[start..];
        // Find opening quote of key.
        let k1 = rest.find('"')? + start;
        let k2 = input[k1 + 1..].find('"')? + k1 + 1;
        let key = &input[k1 + 1..k2];

        // Find colon after key.
        let after_key = &input[k2 + 1..];
        let colon = after_key.find(':')?;
        let value_region = &input[k2 + 1 + colon + 1..];

        // Find opening quote of value.
        let q_offset = value_region.find('"')?;
        let val_start = k2 + 1 + colon + 1 + q_offset + 1;
        let tail = &input[val_start..];

        // Find end of value: next `","` or `"}` boundary.
        let next_key = tail.find("\",\"");
        let end_obj = tail.find("\"}");

        let end = [next_key, end_obj]
            .iter()
            .filter_map(|x| *x)
            .min()
            .unwrap_or(tail.len());

        Some((
            key.to_string(),
            tail[..end].to_string(),
            val_start + end + 1,
        ))
    }

    // -- Stage 6: write -----------------------------------------------------

    fn ultra_recover_write(input: &str) -> Option<serde_json::Value> {
        #[derive(Serialize)]
        struct WriteArgs {
            file_path: String,
            content: String,
        }

        let file_path = ultra_extract_short_field(
            input,
            &[patch_keys::FILE_PATH_SNAKE, patch_keys::FILE_PATH],
        )?;
        // Require content to be present — an empty default would silently
        // overwrite files with nothing, which is worse than failing recovery.
        let content = ultra_extract_large_field(
            input,
            "content",
            &[patch_keys::FILE_PATH_SNAKE, patch_keys::FILE_PATH],
        )?;
        to_json_value(WriteArgs { file_path, content })
    }

    // -- Stage 6: edit ------------------------------------------------------

    fn ultra_recover_edit(input: &str) -> Option<serde_json::Value> {
        #[derive(Serialize)]
        struct EditArgs {
            file_path: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            old_string: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            new_string: Option<String>,
        }

        let file_path = ultra_extract_short_field(
            input,
            &[patch_keys::FILE_PATH_SNAKE, patch_keys::FILE_PATH],
        )?;
        let old_string = ultra_extract_large_field(
            input,
            patch_keys::OLD_STRING,
            &[
                patch_keys::NEW_STRING,
                "newString",
                patch_keys::FILE_PATH_SNAKE,
                patch_keys::FILE_PATH,
            ],
        );
        let new_string = ultra_extract_large_field(
            input,
            patch_keys::NEW_STRING,
            &[
                patch_keys::OLD_STRING,
                "oldString",
                patch_keys::FILE_PATH_SNAKE,
                patch_keys::FILE_PATH,
            ],
        );
        // Also try camelCase variants.
        let old_string = old_string.or_else(|| {
            ultra_extract_large_field(
                input,
                "oldString",
                &[
                    patch_keys::NEW_STRING,
                    "newString",
                    patch_keys::FILE_PATH_SNAKE,
                    patch_keys::FILE_PATH,
                ],
            )
        });
        let new_string = new_string.or_else(|| {
            ultra_extract_large_field(
                input,
                "newString",
                &[
                    patch_keys::OLD_STRING,
                    "oldString",
                    patch_keys::FILE_PATH_SNAKE,
                    patch_keys::FILE_PATH,
                ],
            )
        });

        to_json_value(EditArgs {
            file_path,
            old_string,
            new_string,
        })
    }

    // -- Shared extraction helpers ------------------------------------------

    /// Extract a short, well-formed field value (like a file path).
    /// Stops at the first unescaped `"`.
    /// Uses structure-aware scanning to skip keys that appear inside string values.
    fn ultra_extract_short_field(input: &str, keys: &[&str]) -> Option<String> {
        for key in keys {
            let needle = format!("\"{}\"", key);
            if let Some(idx) = find_top_level_key(input, &needle) {
                let after = &input[idx + needle.len()..];
                let colon = after.find(':')?;
                let rest = &after[colon + 1..];
                let q = rest.find('"')?;
                let val_start = &rest[q + 1..];
                // Read until next unescaped quote.
                if let Some(end) = find_unescaped_quote(val_start) {
                    return Some(val_start[..end].to_string());
                }
            }
        }
        None
    }

    /// Extract a potentially huge field value (like HTML content).
    /// Instead of relying on closing quotes, uses the position of the next
    /// known field key (or `}` at end-of-object) as the boundary.
    /// Uses structure-aware scanning to skip keys that appear inside string values.
    fn ultra_extract_large_field(input: &str, key: &str, stop_keys: &[&str]) -> Option<String> {
        let needle = format!("\"{}\"", key);
        let kpos = find_top_level_key(input, &needle)?;
        let after = &input[kpos + needle.len()..];
        let colon = after.find(':')?;
        let rest = &after[colon + 1..];
        let q = rest.find('"')?;
        let val_abs_start = kpos + needle.len() + colon + 1 + q + 1;
        let tail = &input[val_abs_start..];

        // Find the earliest stop boundary.
        let mut end = tail.len();

        for sk in stop_keys {
            let pat = format!("\"{}\"", sk);
            if let Some(i) = tail.find(&pat) {
                end = end.min(i);
            }
        }
        // Also stop at the last `}` if it's before any stop key.
        if let Some(i) = tail.rfind('}') {
            end = end.min(i);
        }

        Some(ultra_trim_tail(&tail[..end]))
    }

    /// Find a `"key"` pattern at the top level of the JSON structure,
    /// skipping occurrences that appear inside string values.
    /// Returns the byte offset of the match, or None.
    fn find_top_level_key(input: &str, needle: &str) -> Option<usize> {
        let bytes = input.as_bytes();
        let needle_bytes = needle.as_bytes();
        let mut i = 0;
        let mut in_string = false;
        let mut escape = false;

        while i < bytes.len() {
            let b = bytes[i];

            if in_string {
                if escape {
                    escape = false;
                    i += 1;
                    continue;
                }
                if b == b'\\' {
                    escape = true;
                    i += 1;
                    continue;
                }
                if b == b'"' {
                    in_string = false;
                }
                i += 1;
                continue;
            }

            // Outside a string — check for needle match
            if b == b'"' {
                if bytes[i..].starts_with(needle_bytes) {
                    // Verify this looks like a key: preceded by `{`, `,`, or whitespace
                    let prev_significant =
                        bytes[..i].iter().rev().find(|&&c| !c.is_ascii_whitespace());
                    if matches!(prev_significant, Some(b'{') | Some(b',') | None) {
                        return Some(i);
                    }
                }
                // Enter the string (whether it matched or not)
                in_string = true;
            }
            i += 1;
        }
        None
    }

    /// Find the position of the first `"` not preceded by an odd number of `\`.
    fn find_unescaped_quote(s: &str) -> Option<usize> {
        let bytes = s.as_bytes();
        for i in 0..bytes.len() {
            if bytes[i] == b'"' {
                let mut backslashes = 0;
                let mut j = i;
                while j > 0 && bytes[j - 1] == b'\\' {
                    backslashes += 1;
                    j -= 1;
                }
                if backslashes % 2 == 0 {
                    return Some(i);
                }
            }
        }
        None
    }

    /// Trim trailing JSON noise: quotes, commas, whitespace.
    fn ultra_trim_tail(s: &str) -> String {
        let bytes = s.as_bytes();
        let mut end = bytes.len();
        while end > 0 {
            match bytes[end - 1] {
                b'"' | b',' | b'\n' | b'\r' | b' ' | b'\t' => end -= 1,
                _ => break,
            }
        }
        // Safety: we only trimmed single-byte ASCII, so `end` is always a
        // valid UTF-8 boundary.
        s[..end].to_string()
    }
}

pub mod wildcard {
    use glob::Pattern;

    pub fn matches(pattern: &str, text: &str) -> bool {
        Pattern::new(pattern)
            .map(|p| p.matches(text))
            .unwrap_or(false)
    }

    pub fn matches_any(patterns: &[&str], text: &str) -> bool {
        patterns.iter().any(|p| matches(p, text))
    }

    pub fn filter<'a>(pattern: &str, items: &'a [&str]) -> Vec<&'a str> {
        items
            .iter()
            .filter(|s| matches(pattern, s))
            .copied()
            .collect()
    }
}

pub mod color {
    pub fn strip_ansi(s: &str) -> String {
        let re = regex::Regex::new(r"\x1b\[[0-9;]*m").unwrap();
        re.replace_all(s, "").to_string()
    }

    pub fn ansi_length(s: &str) -> usize {
        strip_ansi(s).len()
    }
}

pub mod timeout {
    use std::time::Duration;
    use tokio::time::timeout;

    pub async fn with_timeout<T, F>(duration: Duration, future: F) -> Option<T>
    where
        F: std::future::Future<Output = T>,
    {
        timeout(duration, future).await.ok()
    }
}

pub mod defer {
    pub struct Defer<F: FnOnce()> {
        f: Option<F>,
    }

    impl<F: FnOnce()> Defer<F> {
        pub fn new(f: F) -> Self {
            Self { f: Some(f) }
        }
    }

    impl<F: FnOnce()> Drop for Defer<F> {
        fn drop(&mut self) {
            if let Some(f) = self.f.take() {
                f();
            }
        }
    }

    #[macro_export]
    macro_rules! defer {
        ($($body:expr),*) => {
            let _guard = $crate::defer::Defer::new(move || { $($body);* });
        };
    }
}

pub mod lock {
    use std::sync::Arc;
    use tokio::sync::Mutex;

    pub type AsyncLock<T> = Arc<Mutex<T>>;

    pub fn new<T: Send + 'static>(value: T) -> AsyncLock<T> {
        Arc::new(Mutex::new(value))
    }
}

pub mod token {
    const CHARS_PER_TOKEN: usize = 4;

    pub fn estimate(input: &str) -> usize {
        if input.is_empty() {
            return 0;
        }
        input.len() / CHARS_PER_TOKEN
    }

    pub fn estimate_messages(messages: &[&str]) -> usize {
        messages.iter().map(|m| estimate(m)).sum()
    }
}

pub mod format {
    pub fn format_duration(secs: u64) -> String {
        if secs == 0 {
            return String::new();
        }
        if secs < 60 {
            return format!("{}s", secs);
        }
        if secs < 3600 {
            let mins = secs / 60;
            let remaining = secs % 60;
            if remaining > 0 {
                format!("{}m {}s", mins, remaining)
            } else {
                format!("{}m", mins)
            }
        } else if secs < 86400 {
            let hours = secs / 3600;
            let remaining = (secs % 3600) / 60;
            if remaining > 0 {
                format!("{}h {}m", hours, remaining)
            } else {
                format!("{}h", hours)
            }
        } else if secs < 604800 {
            let days = secs / 86400;
            if days == 1 {
                "~1 day".to_string()
            } else {
                format!("~{} days", days)
            }
        } else {
            let weeks = secs / 604800;
            if weeks == 1 {
                "~1 week".to_string()
            } else {
                format!("~{} weeks", weeks)
            }
        }
    }

    pub fn format_bytes(bytes: u64) -> String {
        const KB: u64 = 1024;
        const MB: u64 = KB * 1024;
        const GB: u64 = MB * 1024;

        if bytes >= GB {
            format!("{:.1} GB", bytes as f64 / GB as f64)
        } else if bytes >= MB {
            format!("{:.1} MB", bytes as f64 / MB as f64)
        } else if bytes >= KB {
            format!("{:.1} KB", bytes as f64 / KB as f64)
        } else {
            format!("{} B", bytes)
        }
    }

    pub fn format_number(n: u64) -> String {
        if n >= 1_000_000 {
            format!("{:.1}M", n as f64 / 1_000_000.0)
        } else if n >= 1_000 {
            format!("{:.1}K", n as f64 / 1_000.0)
        } else {
            n.to_string()
        }
    }
}

pub mod git {
    use std::path::Path;
    use std::process::Command;

    pub struct GitResult {
        pub exit_code: i32,
        pub stdout: String,
        pub stderr: String,
    }

    impl GitResult {
        pub fn text(&self) -> &str {
            &self.stdout
        }

        pub fn success(&self) -> bool {
            self.exit_code == 0
        }
    }

    pub fn run(args: &[&str], cwd: &Path) -> GitResult {
        let output = Command::new("git").args(args).current_dir(cwd).output();

        match output {
            Ok(output) => GitResult {
                exit_code: output.status.code().unwrap_or(1),
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            },
            Err(e) => GitResult {
                exit_code: 1,
                stdout: String::new(),
                stderr: e.to_string(),
            },
        }
    }

    pub fn is_repo(path: &Path) -> bool {
        path.join(".git").exists()
    }

    pub fn get_root(path: &Path) -> Option<std::path::PathBuf> {
        let result = run(&["rev-parse", "--show-toplevel"], path);
        if result.success() {
            Some(std::path::PathBuf::from(result.stdout.trim()))
        } else {
            None
        }
    }

    pub fn get_current_branch(path: &Path) -> Option<String> {
        let result = run(&["branch", "--show-current"], path);
        if result.success() {
            Some(result.stdout.trim().to_string())
        } else {
            None
        }
    }

    pub fn get_remote_url(path: &Path) -> Option<String> {
        let result = run(&["remote", "get-url", "origin"], path);
        if result.success() {
            Some(result.stdout.trim().to_string())
        } else {
            None
        }
    }

    pub fn get_head_commit(path: &Path) -> Option<String> {
        let result = run(&["rev-parse", "HEAD"], path);
        if result.success() {
            Some(result.stdout.trim().to_string())
        } else {
            None
        }
    }

    pub fn get_status(path: &Path) -> Vec<String> {
        let result = run(&["status", "--porcelain"], path);
        if result.success() {
            result.stdout.lines().map(|s| s.to_string()).collect()
        } else {
            Vec::new()
        }
    }

    pub fn has_uncommitted_changes(path: &Path) -> bool {
        !get_status(path).is_empty()
    }
}

pub mod abort {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    #[derive(Clone)]
    pub struct AbortController {
        cancelled: Arc<AtomicBool>,
    }

    impl AbortController {
        pub fn new() -> Self {
            Self {
                cancelled: Arc::new(AtomicBool::new(false)),
            }
        }

        pub fn abort(&self) {
            self.cancelled.store(true, Ordering::SeqCst);
        }

        pub fn is_cancelled(&self) -> bool {
            self.cancelled.load(Ordering::SeqCst)
        }
    }

    impl Default for AbortController {
        fn default() -> Self {
            Self::new()
        }
    }

    pub fn aborted(controller: &AbortController) -> bool {
        controller.is_cancelled()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rocode_core::contracts::tools::BuiltinToolName;

    #[test]
    fn test_token_estimate() {
        assert_eq!(token::estimate(""), 0);
        assert_eq!(token::estimate("hello"), 1);
        assert_eq!(token::estimate("hello world"), 2);
        assert_eq!(token::estimate("a".repeat(100).as_str()), 25);
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format::format_duration(0), "");
        assert_eq!(format::format_duration(30), "30s");
        assert_eq!(format::format_duration(90), "1m 30s");
        assert_eq!(format::format_duration(3600), "1h");
        assert_eq!(format::format_duration(3661), "1h 1m");
        assert_eq!(format::format_duration(86400), "~1 day");
        assert_eq!(format::format_duration(172800), "~2 days");
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format::format_bytes(500), "500 B");
        assert_eq!(format::format_bytes(1024), "1.0 KB");
        assert_eq!(format::format_bytes(1048576), "1.0 MB");
        assert_eq!(format::format_bytes(1073741824), "1.0 GB");
    }

    #[test]
    fn test_wildcard() {
        assert!(wildcard::matches("*.rs", "main.rs"));
        assert!(!wildcard::matches("*.rs", "main.ts"));
        assert!(wildcard::matches_any(&["*.rs", "*.ts"], "main.ts"));
    }

    #[test]
    fn test_color() {
        let input = "\x1b[32mhello\x1b[0m";
        assert_eq!(color::strip_ansi(input), "hello");
        assert_eq!(color::ansi_length(input), 5);
    }

    #[test]
    fn try_parse_json_object_clean() {
        let input = r#"{"file_path":"/tmp/a"}"#;
        let val = json::try_parse_json_object(input).unwrap();
        assert_eq!(val["file_path"], "/tmp/a");
    }

    #[test]
    fn try_parse_json_object_rejects_literal_newline() {
        let input = "{\"file_path\":\"/tmp/a\",\"content\":\"line1\nline2\"}";
        assert!(json::try_parse_json_object(input).is_none());
    }

    #[test]
    fn try_parse_json_object_returns_none_for_non_object() {
        assert!(json::try_parse_json_object("not json at all").is_none());
        assert!(json::try_parse_json_object("42").is_none());
    }

    #[test]
    fn try_parse_json_object_robust_parses_stringified_object() {
        let inner = r#"{"file_path":"/tmp/a","content":"hello"}"#;
        let outer = serde_json::to_string(inner).expect("stringify should succeed");
        let val = json::try_parse_json_object_robust(&outer).expect("should parse object");
        assert_eq!(val["file_path"], "/tmp/a");
        assert_eq!(val["content"], "hello");
    }

    #[test]
    fn try_parse_json_object_robust_parses_bom_wrapped_object() {
        let input = "\u{feff}  {\"file_path\":\"/tmp/a\"}  ";
        let val = json::try_parse_json_object_robust(input).expect("should parse object");
        assert_eq!(val["file_path"], "/tmp/a");
    }

    #[test]
    fn try_parse_json_object_robust_handles_stringified_object_with_literal_controls() {
        let inner_with_literal_newline = "{\"file_path\":\"/tmp/a\",\"content\":\"line1\nline2\"}";
        let outer =
            serde_json::to_string(inner_with_literal_newline).expect("stringify should succeed");
        assert!(json::try_parse_json_object_robust(&outer).is_none());
    }

    #[test]
    fn recover_tool_arguments_from_jsonish_rejects_truncated_write_payload() {
        let malformed = "{\"file_path\":\"/tmp/t2.html\",\"content\":\"<html><body>hello";
        assert!(json::recover_tool_arguments_from_jsonish(
            BuiltinToolName::Write.as_str(),
            malformed
        )
        .is_none());
    }

    #[test]
    fn recover_tool_arguments_from_jsonish_rejects_truncated_bash_payload() {
        let malformed = "{\"command\":\"cat > t2.html << 'EOF'\\n<html>";
        assert!(json::recover_tool_arguments_from_jsonish(
            BuiltinToolName::Bash.as_str(),
            malformed
        )
        .is_none());
    }

    #[test]
    fn recover_tool_arguments_from_jsonish_returns_none_for_unknown_tool() {
        let malformed = "{\"file_path\":\"/tmp/t2.html\",\"content\":\"hello\"";
        assert!(json::recover_tool_arguments_from_jsonish(
            BuiltinToolName::Read.as_str(),
            malformed
        )
        .is_none());
    }

    #[test]
    fn recover_tool_arguments_from_jsonish_rejects_truncated_edit_payload() {
        let malformed = "{\"file_path\":\"/tmp/t2.html\",\"new_string\":\".class { color: red; }";
        assert!(json::recover_tool_arguments_from_jsonish(
            BuiltinToolName::Edit.as_str(),
            malformed
        )
        .is_none());
    }

    #[test]
    fn ultra_returns_none_for_garbage() {
        assert!(
            json::recover_tool_call_ultra(BuiltinToolName::Write.as_str(), "not json at all")
                .is_none()
        );
    }

    #[test]
    fn ultra_fast_path_valid_json() {
        let raw = r#"{"file_path":"/tmp/a","content":"hello"}"#;
        let recovered = json::recover_tool_call_ultra(BuiltinToolName::Write.as_str(), raw)
            .expect("valid JSON should pass through");
        assert_eq!(recovered["file_path"], "/tmp/a");
        assert_eq!(recovered["content"], "hello");
    }
}
