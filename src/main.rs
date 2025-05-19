use std::sync::LazyLock;
use std::time::Instant;

mod display;
mod logos;
mod os;
mod proc;
mod shell;
mod theme;
mod utils;

use utils::{ENV_CACHE, format_memory, format_uptime};

fn main() {
    let start_time = Instant::now();

    let shell_path = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    let version_thread = shell::start_version_detection(&shell_path);

    let theme_thread = theme::start_theme_detection();
    let icon_thread = theme::start_icon_detection();

    LazyLock::force(&ENV_CACHE);

    let mut info = os::collect_system_info();

    let shell_with_version = shell::join_version_thread(version_thread, &shell_path);
    info.shell = shell_with_version;
    info.theme = theme::join_theme_detection_thread(theme_thread);
    info.icons = theme::join_icon_detection_thread(icon_thread);

    // Get the distro name for logo selection
    let os_name_for_logo = info.os_name.split_whitespace().next().unwrap_or("Linux");

    // Find the appropriate logo
    let logo = logos::find_logo(os_name_for_logo)
        .or_else(|| logos::find_logo("Linux"))
        .unwrap_or(&logos::LOGOS[102]);

    let logo_lines: Vec<&str> = logo.ascii_art.lines().collect();
    let padding = 3; // Space between logo and info

    let mut info_lines = Vec::with_capacity(15);
    info_lines.push(format!(
        "{}@{}",
        std::env::var("USER").unwrap_or_else(|_| "user".to_string()),
        info.hostname
    ));
    info_lines.push("-----------------".to_string());
    info_lines.push(format!("OS: {}", info.os_name));
    info_lines.push(format!("Kernel: {}", info.kernel));
    info_lines.push(format!("Uptime: {}", format_uptime(info.uptime)));
    info_lines.push(format!("Shell: {}", info.shell));
    info_lines.push(format!("Resolution: {}", info.resolution));
    info_lines.push(format!("DE: {}", info.de));
    info_lines.push(format!("WM: {}", info.wm));
    info_lines.push(format!("Theme: {}", info.theme));
    info_lines.push(format!("Icons: {}", info.icons));
    info_lines.push(format!("Terminal: {}", info.terminal));
    info_lines.push(format!("CPU: {}", info.cpu_info));
    info_lines.push(format!(
        "Memory: {} / {}",
        format_memory(info.memory_used),
        format_memory(info.memory_total)
    ));

    let max_lines = std::cmp::max(logo_lines.len(), info_lines.len());

    // Track color state
    let mut current_color = String::new();
    let reset_sequence = "\x1b[0m";

    for i in 0..max_lines {
        let logo_line = if i < logo_lines.len() {
            logo_lines[i]
        } else {
            ""
        };
        let info_line = if i < info_lines.len() {
            &info_lines[i]
        } else {
            ""
        };

        // Calculate visible length of the logo line (excluding ANSI escape sequences)
        let mut visible_length = 0;
        let mut in_escape = false;

        for c in logo_line.chars() {
            if c == '\x1b' {
                in_escape = true;
            } else if in_escape && c == 'm' {
                in_escape = false;
            } else if !in_escape {
                visible_length += 1;
            }
        }

        // Print logo line
        print!("{}", logo_line);

        // Parse color sequences in the logo line
        let mut start_idx = 0;

        while let Some(esc_idx) = logo_line[start_idx..].find("\x1b[") {
            let abs_idx = start_idx + esc_idx;

            // Find the end of the sequence (the 'm')
            if let Some(m_idx) = logo_line[abs_idx..].find('m') {
                let end_idx = abs_idx + m_idx + 1;
                let sequence = &logo_line[abs_idx..end_idx];

                if sequence == reset_sequence {
                    current_color.clear();
                } else {
                    current_color = sequence.to_string();
                }

                start_idx = end_idx;
            } else {
                break;
            }
        }

        // Calculate required padding to reach the logo width
        let padding_needed = if visible_length < logo.max_line_length {
            logo.max_line_length - visible_length + padding
        } else {
            padding
        };

        // Print info with padding
        if !info_line.is_empty() {
            // Reset color, add padding, print info
            if !current_color.is_empty() {
                print!(
                    "{}{:padding$}{}",
                    reset_sequence,
                    "",
                    info_line,
                    padding = padding_needed
                );

                // Only restore color if there's more logo lines coming
                if i + 1 < logo_lines.len() {
                    print!("{}", current_color);
                }
            } else {
                print!("{:padding$}{}", "", info_line, padding = padding_needed);
            }
        }

        println!();
    }

    let elapsed = start_time.elapsed();
    eprintln!("Time elapsed: {elapsed:?}");
}
