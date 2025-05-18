use libc::{self, c_char};
use nix::sys::utsname::uname;
use smallvec::{SmallVec, smallvec};
use std::fs::File;
use std::os::fd::AsRawFd;
use std::sync::LazyLock;
use std::thread::{self, JoinHandle};
use std::time::Instant;

mod display;
mod proc;
mod theme;
mod utils;

use utils::{ENV_CACHE, fast_sysinfo, format_memory, format_uptime, get_env_var};

static ARCH_LOGO: &str = r"                    -`                    
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
 .`                                   `/  ";

static DISTRO_NAME: std::sync::LazyLock<String> = std::sync::LazyLock::new(get_distribution_name);

struct SysInfo {
    hostname: String,
    os_name: String,
    kernel: String,
    uptime: u64,
    shell: String,
    terminal: String,
    de: String,
    wm: String,
    theme: String,
    icons: String,
    resolution: String,
    cpu_info: String,
    memory_used: u64,
    memory_total: u64,
}

fn get_cpu_info() -> String {
    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_possible_truncation)]
    let cpu_online = unsafe { libc::sysconf(libc::_SC_NPROCESSORS_ONLN) as usize };

    let mut model_name = String::new();

    if let Ok(file) = File::open("/proc/cpuinfo") {
        const BUF_SIZE: usize = 512;

        let mut buffer = [0u8; BUF_SIZE];

        let fd = file.as_raw_fd();

        let bytes_read =
            unsafe { libc::read(fd, buffer.as_mut_ptr().cast::<libc::c_void>(), BUF_SIZE) };
        #[allow(clippy::cast_sign_loss)]
        if bytes_read > 0 {
            let slice = &buffer[0..bytes_read as usize];

            let model_tag = b"model name\t: ";

            if let Some(pos) = memchr::memmem::find(slice, model_tag) {
                let start = pos + model_tag.len();

                if let Some(end) = memchr::memchr(b'\n', &slice[start..]) {
                    if let Ok(model) = std::str::from_utf8(&slice[start..start + end]) {
                        if let Some(core_idx) = model.find("-Core") {
                            model_name = model[0..core_idx].trim().to_string();
                        } else {
                            model_name = model.trim().to_string();
                        }
                    }
                }
            }
        }
    }

    let mut max_freq_ghz = 0.0;

    if let Ok(freq_str) =
        std::fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/cpuinfo_max_freq")
    {
        #[allow(clippy::cast_precision_loss)]
        if let Ok(freq_khz) = freq_str.trim().parse::<u64>() {
            max_freq_ghz = freq_khz as f64 / 1_000_000.0;
        }
    }

    // Format the result

    if model_name.is_empty() {
        format!("Unknown CPU ({cpu_online} cores)")
    } else {
        let freq_str = if max_freq_ghz > 0.0 {
            format!(" @ {max_freq_ghz:.3}GHz")
        } else {
            String::new()
        };

        format!("{model_name} ({cpu_online}){freq_str}")
    }
}

fn get_memory_info() -> (u64, u64) {
    if let Ok((used, total)) = proc::fast_parse_meminfo() {
        return (used, total);
    }

    // Fallback to sysinfo if our parser fails
    unsafe {
        let info = fast_sysinfo();
        let total = info.totalram * u64::from(info.mem_unit);
        let free = info.freeram * u64::from(info.mem_unit);
        (total - free, total)
    }
}

fn get_distribution_name() -> String {
    if let Ok(file) = File::open("/etc/os-release") {
        if let Ok(mmap) = unsafe { memmap2::MmapOptions::new().map(&file) } {
            let data = mmap.as_ref();

            let name_pattern = b"NAME=";
            let id_pattern = b"ID=";

            if let Some(pos) = memchr::memmem::find(data, name_pattern) {
                let start = pos + name_pattern.len();
                if let Some(end_offset) = memchr::memchr(b'\n', &data[start..]) {
                    let end = start + end_offset;
                    let name = &data[start..end];

                    let name = if name.len() >= 2 && name[0] == b'"' && name[name.len() - 1] == b'"'
                    {
                        &name[1..name.len() - 1]
                    } else {
                        name
                    };

                    if let Ok(name_str) = std::str::from_utf8(name) {
                        return name_str.trim().to_string();
                    }
                }
            } else if let Some(pos) = memchr::memmem::find(data, id_pattern) {
                let start = pos + id_pattern.len();
                if let Some(end_offset) = memchr::memchr(b'\n', &data[start..]) {
                    let end = start + end_offset;
                    if let Ok(id) = std::str::from_utf8(&data[start..end]) {
                        let id = id.trim().trim_matches('"');
                        let mut id_chars = id.chars();
                        return id_chars.next().map_or_else(
                            || "Linux".to_string(),
                            |c| c.to_uppercase().collect::<String>() + id_chars.as_str() + " Linux",
                        );
                    }
                }
            }
        }
    }

    if std::path::Path::new("/etc/arch-release").exists() {
        return "Arch Linux".to_string();
    } else if std::path::Path::new("/etc/debian_version").exists() {
        return "Debian Linux".to_string();
    } else if std::path::Path::new("/etc/redhat-release").exists() {
        return "Red Hat Linux".to_string();
    }

    "Linux".to_string()
}

fn collect_system_info() -> SysInfo {
    let uts = uname().unwrap();

    let sys_info = unsafe { fast_sysinfo() };

    let mut hostname: SmallVec<[u8; 64]> = smallvec![0; 64];
    unsafe {
        libc::gethostname(hostname.as_mut_ptr().cast::<c_char>(), hostname.len());
        let mut i = 0;
        while i < hostname.len() && hostname[i] != 0 {
            i += 1;
        }
        hostname.truncate(i);
    }

    // Extract GPU info if available through environment variables
    // This is much faster than parsing files for Wayland

    #[allow(clippy::cast_sign_loss)]
    let uptime = sys_info.uptime as u64;

    let de = get_env_var("XDG_CURRENT_DESKTOP", "Unknown");

    let wm = match get_env_var("XDG_SESSION_TYPE", "") {
        "wayland" => {
            if de.contains("GNOME") {
                "Mutter"
            } else if de.contains("KDE") {
                "KWin"
            } else {
                "Unknown"
            }
        }
        _ => "Unknown",
    };

    let terminal = get_env_var("TERM", "Unknown");

    let resolution = display::get_screen_resolution();

    let cpu_info = get_cpu_info();

    let (mem_used, mem_total) = get_memory_info();

    let os_name = if uts.sysname().to_string_lossy() == "Linux" {
        format!("{} {}", &*DISTRO_NAME, uts.machine().to_string_lossy())
    } else {
        format!(
            "{} {}",
            uts.sysname().to_string_lossy(),
            uts.machine().to_string_lossy()
        )
    };

    SysInfo {
        hostname: String::from_utf8_lossy(&hostname).into_owned(),
        os_name,
        kernel: uts.release().to_string_lossy().into_owned(),
        uptime,
        shell: String::new(),
        terminal: terminal.to_string(),
        de: de.to_string(),
        wm: wm.to_string(),
        theme: String::new(),
        icons: String::new(),
        resolution,
        cpu_info,
        memory_used: mem_used,
        memory_total: mem_total,
    }
}

fn start_shell_version_detection(shell_path: &str) -> JoinHandle<String> {
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

fn join_shell_version_thread(handle: JoinHandle<String>, shell_path: &str) -> String {
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

fn main() {
    let start_time = Instant::now();

    let shell_path = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    let version_thread = start_shell_version_detection(&shell_path);

    let theme_thread = theme::start_theme_detection();
    let icon_thread = theme::start_icon_detection();

    LazyLock::force(&ENV_CACHE);

    let mut info = collect_system_info();

    let shell_with_version = join_shell_version_thread(version_thread, &shell_path);
    info.shell = shell_with_version;
    info.theme = theme::join_theme_detection_thread(theme_thread);
    info.icons = theme::join_icon_detection_thread(icon_thread);

    let logo_lines: Vec<&str> = ARCH_LOGO.lines().collect();
    let logo_width = logo_lines.iter().map(|line| line.len()).max().unwrap_or(0);
    let padding = 2; // Space between logo and info

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
