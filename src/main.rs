use libc::{self, c_char};
use memmap2::Mmap;
use nix::sys::utsname::uname;
use once_cell::sync::Lazy;
use rustc_hash::FxHashMap;
use smallvec::{SmallVec, smallvec};
use std::ffi::{CStr, CString};
use std::fs::File;
use std::mem::{self};
use std::os::fd::AsRawFd;
use std::time::Instant;

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

static ENV_CACHE: Lazy<FxHashMap<&'static str, String>> = Lazy::new(|| {
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

// Fast implementation of libc sysinfo
#[inline(always)]
unsafe fn fast_sysinfo() -> libc::sysinfo {
    let mut info: libc::sysinfo = unsafe { mem::zeroed() };
    unsafe { libc::sysinfo(&mut info as *mut libc::sysinfo) };
    info
}

fn get_cpu_info() -> String {
    let cpu_online = unsafe { libc::sysconf(libc::_SC_NPROCESSORS_ONLN) as usize };

    let mut model_name = String::new();

    if let Ok(file) = File::open("/proc/cpuinfo") {
        const BUF_SIZE: usize = 512;

        let mut buffer = [0u8; BUF_SIZE];

        let fd = file.as_raw_fd();

        let bytes_read =
            unsafe { libc::read(fd, buffer.as_mut_ptr() as *mut libc::c_void, BUF_SIZE) };

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
        if let Ok(freq_khz) = freq_str.trim().parse::<u64>() {
            // Convert kHz to GHz

            max_freq_ghz = freq_khz as f64 / 1_000_000.0;
        }
    }

    // Format the result

    if model_name.is_empty() {
        format!("Unknown CPU ({} cores)", cpu_online)
    } else {
        let freq_str = if max_freq_ghz > 0.0 {
            format!(" @ {:.3}GHz", max_freq_ghz)
        } else {
            String::new()
        };

        format!("{} ({}){}", model_name, cpu_online, freq_str)
    }
}

#[inline(always)]
unsafe fn find_in_mmap<'a>(mmap: &'a Mmap, pattern: &[u8]) -> Option<&'a [u8]> {
    let data = mmap.as_ref();

    if let Some(idx) = memchr::memmem::find(data, pattern) {
        if let Some(end) = memchr::memchr(b'\n', &data[idx..]) {
            return Some(&data[idx..idx + end]);
        }
    }
    None
}

fn get_memory_info() -> (u64, u64) {
    unsafe {
        let info = fast_sysinfo();
        let total = info.totalram * info.mem_unit as u64;
        let free = info.freeram * info.mem_unit as u64;
        (total - free, total)
    }
}

#[inline(always)]
fn get_env_var<'a>(name: &'a str, default: &'a str) -> &'a str {
    match ENV_CACHE.get(name) {
        Some(val) => val,
        None => default,
    }
}

// Get environment variable from raw C environment
// This is faster than Rust's std::env for repeated lookups
#[allow(dead_code)]
#[inline(always)]
unsafe fn get_raw_env(name: &str) -> Option<String> {
    let c_name = CString::new(name).ok()?;
    let ptr = unsafe { libc::getenv(c_name.as_ptr()) };
    if ptr.is_null() {
        None
    } else {
        Some(unsafe { CStr::from_ptr(ptr).to_string_lossy().into_owned() })
    }
}

fn format_memory(bytes: u64) -> String {
    format!("{} MiB", bytes >> 20)
}

fn collect_system_info() -> SysInfo {
    let uts = uname().unwrap();

    let sys_info = unsafe { fast_sysinfo() };

    let mut hostname: SmallVec<[u8; 64]> = smallvec![0; 64];
    unsafe {
        libc::gethostname(hostname.as_mut_ptr() as *mut c_char, hostname.len());
        let mut i = 0;
        while i < hostname.len() && hostname[i] != 0 {
            i += 1;
        }
        hostname.truncate(i);
    }

    // Extract GPU info if available through environment variables
    // This is much faster than parsing files for Wayland

    let shell = get_env_var("SHELL", "/bin/sh");
    let shell_name = if let Some(idx) = shell.rfind('/') {
        &shell[idx + 1..]
    } else {
        shell
    };

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

    let theme = get_env_var("GTK_THEME", "Unknown");
    let icons = get_env_var("ICON_THEME", "Unknown");

    let terminal = get_env_var("TERM", "Unknown");

    let resolution = match get_env_var("XDG_SESSION_TYPE", "") {
        "wayland" => "Wayland Session",
        _ => "Unknown",
    };

    let cpu_info = get_cpu_info();

    let (mem_used, mem_total) = get_memory_info();

    SysInfo {
        hostname: String::from_utf8_lossy(&hostname).into_owned(),
        os_name: format!(
            "{} {}",
            uts.sysname().to_string_lossy(),
            uts.machine().to_string_lossy()
        ),
        kernel: uts.release().to_string_lossy().into_owned(),
        uptime,
        shell: shell_name.to_string(),
        terminal: terminal.to_string(),
        de: de.to_string(),
        wm: wm.to_string(),
        theme: theme.to_string(),
        icons: icons.to_string(),
        resolution: resolution.to_string(),
        cpu_info,
        memory_used: mem_used,
        memory_total: mem_total,
    }
}

fn format_uptime(seconds: u64) -> String {
    let mins = seconds / 60;
    if mins < 60 {
        return format!("{} mins", mins);
    }

    let hours = mins / 60;
    let mins = mins % 60;
    if hours < 24 {
        return format!("{}h {}m", hours, mins);
    }

    let days = hours / 24;
    let hours = hours % 24;
    format!("{}d {}h {}m", days, hours, mins)
}

fn main() {
    let start_time = Instant::now();

    Lazy::force(&ENV_CACHE);

    let info = collect_system_info();

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
    eprintln!("Time elapsed: {:?}", elapsed);
}
