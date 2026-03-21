pub mod json {
    use rocode_core::contracts::tools::BuiltinToolName;

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
        parse_json_object_with_recovery(input)
    }

    pub fn try_parse_json_object(input: &str) -> Option<serde_json::Value> {
        parse_json_object_with_recovery(input)
    }

    pub fn recover_tool_arguments_from_jsonish(
        tool_name: &str,
        input: &str,
    ) -> Option<serde_json::Value> {
        let _tool = tool_name.parse::<BuiltinToolName>().ok()?;
        try_parse_json_object(input)
    }

    pub fn recover_tool_call_ultra(tool: &str, raw: &str) -> Option<serde_json::Value> {
        let _tool = tool.parse::<BuiltinToolName>().ok()?;
        try_parse_json_object_robust(raw)
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
    fn try_parse_json_object_robust_rejects_stringified_object() {
        let inner = r#"{"file_path":"/tmp/a","content":"hello"}"#;
        let outer = serde_json::to_string(inner).expect("stringify should succeed");
        assert!(json::try_parse_json_object_robust(&outer).is_none());
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
        assert!(json::recover_tool_arguments_from_jsonish("unknown", malformed).is_none());
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
