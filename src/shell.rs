use crate::utils;
use std::thread::{self, JoinHandle};

/// Start shell version detection in separate thread
pub fn start_version_detection(shell_path: &str) -> JoinHandle<String> {
    let shell_path = shell_path.to_string();

    #[allow(clippy::option_if_let_else)]
    thread::spawn(move || {
        let shell_name = if let Some(idx) = shell_path.rfind('/') {
            &shell_path[idx + 1..]
        } else {
            &shell_path
        };

        match shell_name {
            "zsh" => detect_zsh_version(),
            "bash" => detect_bash_version(),
            "fish" => detect_fish_version(),
            _ => shell_name.to_string(),
        }
    })
}

pub fn join_version_thread(handle: JoinHandle<String>, shell_path: &str) -> String {
    handle.join().unwrap_or_else(|_| {
        let shell_name = shell_path
            .rfind('/')
            .map_or(shell_path, |idx| &shell_path[idx + 1..]);
        shell_name.to_string()
    })
}

fn detect_zsh_version() -> String {
    if let Some(output) = utils::run_command("zsh", &["--version"]) {
        let first_line = output.lines().next().unwrap_or("");

        if let Some(pos) = first_line.find("zsh ") {
            let version_start = pos + 4;
            if let Some(pos) = first_line[version_start..].find(' ') {
                return format!("zsh {}", &first_line[version_start..version_start + pos]);
            }
        }
    }
    "zsh".to_string()
}

fn detect_bash_version() -> String {
    if let Some(output) = utils::run_command("bash", &["--version"]) {
        let first_line = output.lines().next().unwrap_or("");

        if let Some(pos) = first_line.find("version ") {
            let version_start = pos + 8;
            if let Some(pos) = first_line[version_start..].find(['-', '(']) {
                let version = first_line[version_start..version_start + pos].trim();
                return format!("bash {version}");
            }
            let remaining = first_line[version_start..]
                .split_whitespace()
                .next()
                .unwrap_or("");
            if !remaining.is_empty() {
                return format!("bash {remaining}");
            }
        }
    }
    "bash".to_string()
}

fn detect_fish_version() -> String {
    if let Some(output) = utils::run_command("fish", &["--version"]) {
        let first_line = output.lines().next().unwrap_or("");

        if let Some(pos) = first_line.find("version ") {
            let version_start = pos + 8;
            let version = first_line[version_start..].trim();
            if !version.is_empty() {
                return format!("fish {version}");
            }
        }
    }
    "fish".to_string()
}
