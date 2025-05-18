use crate::utils::file_exists;
use std::fs;
use std::path::Path;

// EDID constants
const EDID_HEADER: [u8; 8] = [0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00];
const EDID_SIZE: usize = 128;

/// Cache display resolutions to avoid repeated parsing
pub fn get_screen_resolution() -> String {
    if let Ok(resolution) = get_drm_resolution() {
        return resolution;
    }

    "Unknown".to_string()
}

/// Get all display resolutions from DRM/EDID
fn get_drm_resolution() -> Result<String, ()> {
    let drm_path = Path::new("/sys/class/drm");
    if !drm_path.exists() {
        return Err(());
    }

    let mut resolutions = rustc_hash::FxHashMap::default();
    let mut active_connectors = smallvec::SmallVec::<[std::path::PathBuf; 4]>::new();

    // First find all potential connectors
    if let Ok(entries) = fs::read_dir(drm_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            let file_name = path.file_name().unwrap_or_default().to_string_lossy();

            // Look for card outputs like card0-HDMI-A-1
            if file_name.starts_with("card") && file_name.contains('-') {
                let status_path = path.join("status");
                let edid_path = path.join("edid");

                // Check if connected and has EDID data
                if file_exists(&status_path) && file_exists(&edid_path) {
                    if let Ok(status) = fs::read_to_string(&status_path) {
                        if status.trim() == "connected" {
                            active_connectors.push(path);
                        }
                    }
                }
            }
        }
    }

    // Read EDID for each active connector
    for path in active_connectors {
        let edid_path = path.join("edid");
        if let Ok(edid_data) = fs::read(&edid_path) {
            if let Some(resolution) = parse_edid_resolution(&edid_data) {
                let connector_name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                resolutions.insert(connector_name, resolution);
            }
        }
    }

    // Combine all resolutions
    if !resolutions.is_empty() {
        let mut result = String::new();
        for (i, (_, res)) in resolutions.iter().enumerate() {
            if i > 0 {
                result.push_str(", ");
            }
            result.push_str(res);
        }
        return Ok(result);
    }

    Err(())
}

/// Parse EDID data to extract resolution
/// The resolution is stored in bytes 54-61 of the EDID data
fn parse_edid_resolution(edid: &[u8]) -> Option<String> {
    // Validate EDID size and header
    if edid.len() < EDID_SIZE || &edid[0..8] != EDID_HEADER.as_ref() {
        return None;
    }

    // Horizontal resolution: low 8 bits in byte 56, high 4 bits in upper nibble of byte 58
    let h_res = ((u16::from(edid[58]) & 0xF0) << 4) + u16::from(edid[56]);

    // Vertical resolution: low 8 bits in byte 59, high 4 bits in upper nibble of byte 61
    let v_res = ((u16::from(edid[61]) & 0xF0) << 4) + u16::from(edid[59]);

    if h_res > 0 && v_res > 0 {
        return Some(format!("{h_res}x{v_res}"));
    }

    None
}
