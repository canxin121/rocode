//! Application branding — logo art, name, version, tagline.
//!
//! Shared between CLI and TUI so neither needs to depend on the other
//! for branding constants.

/// Block-character logo art (3 lines).
pub const LOGO: [&str; 3] = [
    "█▀▀█ █▀▀█ █▀▀ ▄▄▄▄ ▄▄▄█ ▄▄▄▄",
    "█▀█▀ █  █ █   █  █ █  █ █■■■",
    "▀ ▀▀ ▀▀▀▀ ▀▀▀ ▀▀▀▀ ▀▀▀▀ ▀▀▀▀",
];

/// Return logo lines, each prefixed by `pad`.
pub fn logo_lines(pad: &str) -> Vec<String> {
    LOGO.iter().map(|line| format!("{pad}{line}")).collect()
}
