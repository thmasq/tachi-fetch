//! Ultra-fast proc filesystem parser
//! This provides zero-copy, zero-allocation parsers for /proc
//! and can extract values with just a single pass through the file

use memmap2::{Mmap, MmapOptions};
use std::fs::File;
use std::io::{Error, Result};

/// Opens and memory maps a proc file for maximum performance
#[inline(always)]
pub fn mmap_proc_file(path: &str) -> Result<Mmap> {
    let file = File::open(path)?;
    unsafe { MmapOptions::new().map(&file) }
}

/// Fast specialized parser for CPU info
/// Returns the model name and frequency with zero-copy and minimal allocation
pub fn parse_cpu_info(mmap: &Mmap) -> (String, String) {
    let data = unsafe { std::slice::from_raw_parts(mmap.as_ptr(), mmap.len()) };

    // Use byte-based search for maximum performance
    let model_tag = b"model name\t: ";
    let mhz_tag = b"cpu MHz\t\t: ";

    let mut model_name = String::new();
    let mut cpu_mhz = String::new();

    let mut pos = 0;
    while pos < data.len() {
        // Search for model name
        if model_name.is_empty()
            && pos + model_tag.len() < data.len()
            && &data[pos..pos + model_tag.len()] == model_tag
        {
            pos += model_tag.len();
            let start = pos;
            // Find end of line
            while pos < data.len() && data[pos] != b'\n' {
                pos += 1;
            }
            // Extract the model name with minimal copying
            if let Ok(s) = std::str::from_utf8(&data[start..pos]) {
                model_name = s.to_string();
            }
            continue;
        }

        // Search for CPU frequency
        if cpu_mhz.is_empty()
            && pos + mhz_tag.len() < data.len()
            && &data[pos..pos + mhz_tag.len()] == mhz_tag
        {
            pos += mhz_tag.len();
            let start = pos;
            // Find end of line
            while pos < data.len() && data[pos] != b'\n' {
                pos += 1;
            }
            // Extract the frequency with minimal copying
            if let Ok(s) = std::str::from_utf8(&data[start..pos]) {
                // Convert MHz to GHz with minimal parsing
                if let Ok(mhz) = s.trim().parse::<f32>() {
                    cpu_mhz = format!("{:.3} GHz", mhz / 1000.0);
                } else {
                    cpu_mhz = s.to_string();
                }
            }
            continue;
        }

        // Skip to next line for faster processing
        while pos < data.len() && data[pos] != b'\n' {
            pos += 1;
        }
        pos += 1;
    }

    (model_name, cpu_mhz)
}

/// Fast specialized parser for memory info
/// Returns used and total memory in bytes with minimal allocation
pub fn parse_meminfo(mmap: &Mmap) -> (u64, u64) {
    let data = unsafe { std::slice::from_raw_parts(mmap.as_ptr(), mmap.len()) };

    // Tags to look for
    let mem_total_tag = b"MemTotal:";
    let mem_free_tag = b"MemFree:";
    let mem_available_tag = b"MemAvailable:";

    let mut total: u64 = 0;
    let mut free: u64 = 0;
    let mut available: u64 = 0;

    let mut pos = 0;
    while pos < data.len() {
        // Search for MemTotal
        if total == 0
            && pos + mem_total_tag.len() < data.len()
            && &data[pos..pos + mem_total_tag.len()] == mem_total_tag
        {
            pos += mem_total_tag.len();
            // Skip whitespace
            while pos < data.len() && (data[pos] == b' ' || data[pos] == b'\t') {
                pos += 1;
            }
            // Parse the number
            let start = pos;
            while pos < data.len() && data[pos] >= b'0' && data[pos] <= b'9' {
                pos += 1;
            }
            if let Ok(s) = std::str::from_utf8(&data[start..pos]) {
                if let Ok(val) = s.parse::<u64>() {
                    // Convert from kB to bytes
                    total = val * 1024;
                }
            }
            continue;
        }

        // Search for MemAvailable (preferred) or MemFree
        if available == 0
            && pos + mem_available_tag.len() < data.len()
            && &data[pos..pos + mem_available_tag.len()] == mem_available_tag
        {
            pos += mem_available_tag.len();
            // Skip whitespace
            while pos < data.len() && (data[pos] == b' ' || data[pos] == b'\t') {
                pos += 1;
            }
            // Parse the number
            let start = pos;
            while pos < data.len() && data[pos] >= b'0' && data[pos] <= b'9' {
                pos += 1;
            }
            if let Ok(s) = std::str::from_utf8(&data[start..pos]) {
                if let Ok(val) = s.parse::<u64>() {
                    // Convert from kB to bytes
                    available = val * 1024;
                }
            }
            continue;
        }

        // Search for MemFree (fallback if MemAvailable not found)
        if free == 0
            && pos + mem_free_tag.len() < data.len()
            && &data[pos..pos + mem_free_tag.len()] == mem_free_tag
        {
            pos += mem_free_tag.len();
            // Skip whitespace
            while pos < data.len() && (data[pos] == b' ' || data[pos] == b'\t') {
                pos += 1;
            }
            // Parse the number
            let start = pos;
            while pos < data.len() && data[pos] >= b'0' && data[pos] <= b'9' {
                pos += 1;
            }
            if let Ok(s) = std::str::from_utf8(&data[start..pos]) {
                if let Ok(val) = s.parse::<u64>() {
                    // Convert from kB to bytes
                    free = val * 1024;
                }
            }
            continue;
        }

        // Skip to next line for faster processing
        while pos < data.len() && data[pos] != b'\n' {
            pos += 1;
        }
        pos += 1;

        // Early exit if we have all the data we need
        if total > 0 && (available > 0 || free > 0) {
            break;
        }
    }

    let free_mem = if available > 0 { available } else { free };
    let used_mem = if total > free_mem {
        total - free_mem
    } else {
        0
    };

    (used_mem, total)
}

/// Count the number of directories in a specific path
/// For example, to count packages from pacman
pub fn count_directories(path: &str) -> Result<usize> {
    let mut count = 0;

    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_dir() {
                    count += 1;
                }
            }
        }
    }

    Ok(count)
}
