use crate::display;
use crate::proc;
use crate::utils::{fast_sysinfo, get_env_var};
use libc::{self, c_char};
use nix::sys::utsname::uname;
use smallvec::{SmallVec, smallvec};
use std::fs::File;
use std::os::fd::AsRawFd;
use std::sync::LazyLock;

pub struct SysInfo {
    pub hostname: String,
    pub os_name: String,
    pub kernel: String,
    pub uptime: u64,
    pub shell: String,
    pub terminal: String,
    pub de: String,
    pub wm: String,
    pub theme: String,
    pub icons: String,
    pub resolution: String,
    pub cpu_info: String,
    pub memory_used: u64,
    pub memory_total: u64,
}

static DISTRO_NAME: LazyLock<String> = LazyLock::new(get_distribution_name);

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

pub fn get_cpu_info() -> String {
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
                        let trimmed_model = model.trim();

                        // Look for "-Core" pattern
                        if let Some(core_idx) =
                            memchr::memmem::find(trimmed_model.as_bytes(), b"-Core")
                        {
                            // Find the last space before "-Core"
                            let prefix_slice = &trimmed_model.as_bytes()[..core_idx];

                            // Try to find the last space before the core count
                            if let Some(last_space) = memchr::memrchr(b' ', prefix_slice) {
                                // Check if everything between the last space and "-Core" is numeric
                                let potential_count = &prefix_slice[last_space + 1..];
                                let is_numeric =
                                    potential_count.iter().all(|&b| b >= b'0' && b <= b'9');

                                if is_numeric && !potential_count.is_empty() {
                                    // This is a format like "AMD Ryzen 7 7800X3D 8-Core"
                                    model_name = trimmed_model[..last_space].to_string();
                                } else {
                                    // This is a format like "AMD EPYC 7773X 64-Core"
                                    model_name = trimmed_model[..core_idx].to_string();
                                }
                            } else {
                                // No space found, use everything before "-Core"
                                model_name = trimmed_model[..core_idx].to_string();
                            }
                        } else {
                            model_name = trimmed_model.to_string();
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

pub fn get_memory_info() -> (u64, u64) {
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

pub fn collect_system_info() -> SysInfo {
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
