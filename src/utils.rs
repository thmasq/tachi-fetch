use libc::{self};
use rustc_hash::FxHashMap;
use std::ffi::{CStr, CString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::LazyLock;

// File and path utilities

/// Fast utility to check if file exists
#[allow(clippy::inline_always)]
#[inline(always)]
pub fn file_exists(path: &Path) -> bool {
    Path::new(path).exists()
}

/// Expand ~ to home directory
pub fn expand_path(path: &str) -> PathBuf {
    if let Some(stripped) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(stripped);
        }
    }
    PathBuf::from(path)
}

// Environment variable utilities

/// Environment variable cache to avoid repeated lookups
pub static ENV_CACHE: LazyLock<FxHashMap<&'static str, String>> = LazyLock::new(|| {
    let mut map = FxHashMap::default();
    for var in &[
        "XDG_CURRENT_DESKTOP",
        "XDG_SESSION_TYPE",
        "SHELL",
        "TERM",
        "WAYLAND_DISPLAY",
        "DISPLAY",
        "DESKTOP_SESSION",
        "GTK_THEME",
        "ICON_THEME",
    ] {
        if let Ok(val) = std::env::var(*var) {
            map.insert(*var, val);
        }
    }
    map
});

/// Get environment variable from cache with default value
#[allow(clippy::inline_always)]
#[inline(always)]
pub fn get_env_var<'a>(name: &'a str, default: &'a str) -> &'a str {
    ENV_CACHE.get(name).map_or(default, |val| val)
}

/// Get environment variable from raw C environment
/// This is faster than Rust's `std::env` for repeated lookups
#[allow(dead_code)]
#[allow(clippy::inline_always)]
#[inline(always)]
pub unsafe fn get_raw_env(name: &str) -> Option<String> {
    let c_name = CString::new(name).ok()?;
    let ptr = unsafe { libc::getenv(c_name.as_ptr()) };
    if ptr.is_null() {
        None
    } else {
        Some(unsafe { CStr::from_ptr(ptr).to_string_lossy().into_owned() })
    }
}

// Command utilities

/// Execute a command and return its trimmed output if successful
pub fn run_command(cmd: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(cmd).args(args).output().ok()?;

    if output.status.success() {
        let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !value.is_empty() {
            return Some(value);
        }
    }
    None
}

// File parsing utilities

/// Efficient file line search - stop after finding what we need
pub fn search_file_for_key(path: &Path, key: &str) -> Option<String> {
    if !file_exists(path) {
        return None;
    }

    if let Ok(content) = fs::read_to_string(path) {
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with(key) && line.contains('=') {
                let parts: Vec<&str> = line.splitn(2, '=').collect();
                if parts.len() == 2 {
                    let value = parts[1].trim().trim_matches('"');
                    if !value.is_empty() {
                        return Some(value.to_string());
                    }
                }
            }
        }
    }
    None
}

// Binary search utilities

/// Check if a byte sequence matches at a position
#[allow(clippy::inline_always)]
#[inline(always)]
pub fn matches_at(data: &[u8], pattern: &[u8]) -> bool {
    data.len() >= pattern.len() && &data[..pattern.len()] == pattern
}

/// Parse a number after a specific offset in the data
#[allow(clippy::inline_always)]
#[inline(always)]
pub fn parse_number_after(data: &[u8], offset: usize) -> Option<(u64, usize)> {
    let mut pos = offset;

    while pos < data.len() && (data[pos] == b' ' || data[pos] == b'\t') {
        pos += 1;
    }

    let start = pos;
    let mut value: u64 = 0;

    while pos < data.len() && data[pos] >= b'0' && data[pos] <= b'9' {
        value = value * 10 + u64::from(data[pos] - b'0');
        pos += 1;
    }

    if pos > start {
        Some((value, pos))
    } else {
        None
    }
}

// Formatting utilities

/// Format byte size to MiB
pub fn format_memory(bytes: u64) -> String {
    format!("{} MiB", bytes >> 20)
}

/// Format seconds to a human-readable uptime string
pub fn format_uptime(seconds: u64) -> String {
    let mins = seconds / 60;
    if mins < 60 {
        return format!("{mins} mins");
    }

    let hours = mins / 60;
    let mins = mins % 60;
    if hours < 24 {
        return format!("{hours}h {mins}m");
    }

    let days = hours / 24;
    let hours = hours % 24;
    format!("{days}d {hours}h {mins}m")
}

// System info utilities

/// Fast sysinfo call
#[allow(clippy::inline_always)]
#[inline(always)]
pub unsafe fn fast_sysinfo() -> libc::sysinfo {
    let mut info: libc::sysinfo = unsafe { std::mem::zeroed() };
    unsafe { libc::sysinfo(&raw mut info) };
    info
}
