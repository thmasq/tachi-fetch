//! Ultra-fast proc filesystem parser
//! This provides zero-copy, zero-allocation parsers for /proc
//! and can extract values with just a single pass through the file

use crate::utils::{matches_at, parse_number_after};
use std::fs::File;
use std::io::Result;

const REQUIRED: usize = 6;

/// Fast specialized parser for memory info
/// Returns used and total memory in bytes according to the formula:
/// Used = Total - Free - Buffers - Cached - `SReclaimable` + Shmem
pub fn fast_parse_meminfo() -> Result<(u64, u64)> {
    let mut buffer = [0u8; 4096];
    let mut file = File::open("/proc/meminfo")?;

    let bytes_read = std::io::Read::read(&mut file, &mut buffer)?;
    if bytes_read == 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "Empty file",
        ));
    }

    let mut total: u64 = 0;
    let mut free: u64 = 0;
    let mut buffers: u64 = 0;
    let mut cached: u64 = 0;
    let mut sreclaimable: u64 = 0;
    let mut shmem: u64 = 0;

    let total_pattern = b"MemTotal:";
    let free_pattern = b"MemFree:";
    let buffers_pattern = b"Buffers:";
    let cached_pattern = b"Cached:";
    let sreclaimable_pattern = b"SReclaimable:";
    let shmem_pattern = b"Shmem:";

    let mut pos = 0;
    let mut found = 0;

    while pos < bytes_read && found < REQUIRED {
        if total == 0 && matches_at(&buffer[pos..], total_pattern) {
            if let Some((value, new_pos)) = parse_number_after(&buffer[pos..], total_pattern.len())
            {
                total = value;
                pos += new_pos;
                found += 1;
                continue;
            }
        } else if free == 0 && matches_at(&buffer[pos..], free_pattern) {
            if let Some((value, new_pos)) = parse_number_after(&buffer[pos..], free_pattern.len()) {
                free = value;
                pos += new_pos;
                found += 1;
                continue;
            }
        } else if buffers == 0 && matches_at(&buffer[pos..], buffers_pattern) {
            if let Some((value, new_pos)) =
                parse_number_after(&buffer[pos..], buffers_pattern.len())
            {
                buffers = value;
                pos += new_pos;
                found += 1;
                continue;
            }
        } else if cached == 0 && matches_at(&buffer[pos..], cached_pattern) {
            if pos == 0 || buffer[pos - 1] == b'\n' {
                if let Some((value, new_pos)) =
                    parse_number_after(&buffer[pos..], cached_pattern.len())
                {
                    cached = value;
                    pos += new_pos;
                    found += 1;
                    continue;
                }
            }
        } else if sreclaimable == 0 && matches_at(&buffer[pos..], sreclaimable_pattern) {
            if let Some((value, new_pos)) =
                parse_number_after(&buffer[pos..], sreclaimable_pattern.len())
            {
                sreclaimable = value;
                pos += new_pos;
                found += 1;
                continue;
            }
        } else if shmem == 0 && matches_at(&buffer[pos..], shmem_pattern) {
            if let Some((value, new_pos)) = parse_number_after(&buffer[pos..], shmem_pattern.len())
            {
                shmem = value;
                pos += new_pos;
                found += 1;
                continue;
            }
        }

        if let Some(nl_pos) = memchr::memchr(b'\n', &buffer[pos..bytes_read]) {
            pos += nl_pos + 1;
        } else {
            break;
        }
    }

    let total_bytes = total << 10;
    let adjusted_used = if total > 0 {
        let non_used = free + buffers + cached + sreclaimable;
        let base_used = total.saturating_sub(non_used);
        base_used + shmem
    } else {
        0
    };

    let used_bytes = adjusted_used * 1024;

    Ok((used_bytes, total_bytes))
}
