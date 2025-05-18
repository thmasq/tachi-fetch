use std::sync::LazyLock;
use std::time::Instant;

mod display;
mod os;
mod proc;
mod shell;
mod theme;
mod utils;

use utils::{ENV_CACHE, format_memory, format_uptime};

static ARCH_LOGO: &str = r"                   -`
                  .o+`
                 `ooo/
                `+oooo:
               `+oooooo:
               -+oooooo+:
             `/:-:++oooo+:
            `/++++/+++++++:
           `/++++++++++++++:
          `/+++ooooooooooooo/`
         ./ooosssso++osssssso+`
        .oossssso-````/ossssss+`
       -osssssso.      :ssssssso.
      :osssssss/        osssso+++.
     /ossssssss/        +ssssooo/-
   `/ossssso+/:-        -:/+osssso+-
  `+sso+:-`                 `.-/+oso:
 `++:.                           `-/+/
 .`                                 `/";

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

    let logo_lines: Vec<&str> = ARCH_LOGO.lines().collect();
    let logo_width = logo_lines.iter().map(|line| line.len()).max().unwrap_or(0);
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

        println!(
            "{:width$}{:padding$}{}",
            logo_line,
            "",
            info_line,
            width = logo_width,
            padding = padding
        );
    }

    let elapsed = start_time.elapsed();
    eprintln!("Time elapsed: {elapsed:?}");
}
