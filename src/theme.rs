use crate::utils::{expand_path, run_command, search_file_for_key};
use std::thread::{self, JoinHandle};

// Paths where theme and icon configurations might be found
static THEME_CONFIG_PATHS: &[&str] = &[
    "~/.gtkrc-2.0",
    "~/.config/gtk-3.0/settings.ini",
    "~/.config/gtk-4.0/settings.ini",
    "/etc/gtk-3.0/settings.ini",
    "/etc/gtk-4.0/settings.ini",
];

static ICON_CONFIG_PATHS: &[&str] = &[
    "~/.config/gtk-3.0/settings.ini",
    "~/.config/gtk-4.0/settings.ini",
    "/etc/gtk-3.0/settings.ini",
    "/etc/gtk-4.0/settings.ini",
    "~/.icons/default/index.theme",
    "/usr/share/icons/default/index.theme",
];

// Try to detect using dconf/gsettings for GNOME-based environments
fn query_gsettings(schema: &str, key: &str) -> Option<String> {
    run_command("gsettings", &["get", schema, key])
}

// Try to detect using kf5-config for KDE
fn query_kde_config(group: &str, key: &str) -> Option<String> {
    // First try kreadconfig5
    if let Some(value) = run_command("kreadconfig5", &["--group", group, "--key", key]) {
        return Some(value);
    }

    // Fall back to kreadconfig
    run_command("kreadconfig", &["--group", group, "--key", key])
}

// Query XSETTINGS for Xfce and other desktops
fn query_xsettings(property: &str) -> Option<String> {
    run_command("xfconf-query", &["-c", "xsettings", "-p", property])
}

// Internal theme detection function
fn detect_gtk_theme_internal() -> String {
    // 1. First check environment variables (as you did)
    if let Ok(theme) = std::env::var("GTK_THEME") {
        if !theme.is_empty() {
            return theme;
        }
    }

    // 2. Try desktop environment specific methods
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let desktop_lower = desktop.to_lowercase();

    // For GNOME, Cinnamon, Budgie, etc.
    if desktop_lower.contains("gnome")
        || desktop_lower.contains("budgie")
        || desktop_lower.contains("cinnamon")
        || desktop_lower.contains("unity")
    {
        if let Some(theme) = query_gsettings("org.gnome.desktop.interface", "gtk-theme") {
            return theme;
        }
    }

    // For KDE Plasma
    if desktop_lower.contains("kde") {
        if let Some(theme) = query_kde_config("KDE", "widgetStyle") {
            return theme;
        }
    }

    // For Xfce
    if desktop_lower.contains("xfce") {
        if let Some(theme) = query_xsettings("/Net/ThemeName") {
            return theme;
        }
    }

    // 3. Check config files
    for path_str in THEME_CONFIG_PATHS {
        let path = expand_path(path_str);

        // For .ini style files
        if path.extension().is_some_and(|ext| ext == "ini") {
            if let Some(theme) = search_file_for_key(&path, "gtk-theme-name") {
                return theme;
            }
        }
        // For gtk2 style files
        else if path.file_name().is_some_and(|name| name == ".gtkrc-2.0") {
            if let Ok(content) = std::fs::read_to_string(&path) {
                for line in content.lines() {
                    if line.contains("gtk-theme-name") {
                        let parts: Vec<&str> = line.split('=').collect();
                        if parts.len() > 1 {
                            let theme = parts[1].trim().trim_matches('"');
                            if !theme.is_empty() {
                                return theme.to_string();
                            }
                        }
                    }
                }
            }
        }
    }

    // If nothing found, return Unknown
    "Unknown".to_string()
}

// Internal icon theme detection function
fn detect_icon_theme_internal() -> String {
    // 1. First check environment variables
    if let Ok(icons) = std::env::var("ICON_THEME") {
        if !icons.is_empty() {
            return icons;
        }
    }

    // 2. Try desktop environment specific methods
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let desktop_lower = desktop.to_lowercase();

    // For GNOME, Cinnamon, Budgie, etc.
    if desktop_lower.contains("gnome")
        || desktop_lower.contains("budgie")
        || desktop_lower.contains("cinnamon")
        || desktop_lower.contains("unity")
    {
        if let Some(icons) = query_gsettings("org.gnome.desktop.interface", "icon-theme") {
            return icons;
        }
    }

    // For KDE Plasma
    if desktop_lower.contains("kde") {
        if let Some(icons) = query_kde_config("Icons", "Theme") {
            return icons;
        }
    }

    // For Xfce
    if desktop_lower.contains("xfce") {
        if let Some(icons) = query_xsettings("/Net/IconThemeName") {
            return icons;
        }
    }

    // 3. Check config files
    for path_str in ICON_CONFIG_PATHS {
        let path = expand_path(path_str);

        // For .ini style files
        if path.extension().is_some_and(|ext| ext == "ini") {
            if let Some(icons) = search_file_for_key(&path, "gtk-icon-theme-name") {
                return icons;
            }
        }
        // For index.theme files
        else if path.file_name().is_some_and(|name| name == "index.theme") {
            if let Ok(content) = std::fs::read_to_string(&path) {
                for line in content.lines() {
                    if line.starts_with("Inherits=") {
                        let icons = line.trim_start_matches("Inherits=").trim();
                        if !icons.is_empty() {
                            return icons.to_string();
                        }
                    }
                }
            }
        }
    }

    // If nothing found, return Unknown
    "Unknown".to_string()
}

/// Start theme detection in separate thread
pub fn start_theme_detection() -> JoinHandle<String> {
    thread::spawn(detect_gtk_theme_internal)
}

/// Start icon theme detection in separate thread
pub fn start_icon_detection() -> JoinHandle<String> {
    thread::spawn(detect_icon_theme_internal)
}

/// Join theme detection thread and handle errors
pub fn join_theme_detection_thread(handle: JoinHandle<String>) -> String {
    handle.join().unwrap_or_else(|_| "Unknown".to_string())
}

/// Join icon detection thread and handle errors
pub fn join_icon_detection_thread(handle: JoinHandle<String>) -> String {
    handle
        .join().unwrap_or_else(|_| "Unknown".to_string())
}
