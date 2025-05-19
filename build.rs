use regex::Regex;
use std::env;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;

#[derive(Debug)]
struct Logo {
    name: String,
    is_wildcard: bool,
    colors: Vec<u8>,
    ascii_art: String,
}

#[derive(Debug)]
struct ProcessedLogo {
    name: String,
    is_wildcard: bool,
    ascii_art: String,
    max_line_length: usize,
}

fn main() -> io::Result<()> {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("Failed to get CARGO_MANIFEST_DIR");
    let neofetch_source = "./logos/logos.txt";
    let output_path = Path::new(&manifest_dir).join("src").join("logos.rs");

    // Read source file
    let source_content = fs::read_to_string(neofetch_source)?;

    // Extract logo definitions
    let logos = extract_logos(&source_content);
    println!("Found {} logo definitions", logos.len());

    // Pre-process logos to include ANSI color codes directly
    let processed_logos = process_logos(&logos);

    // Generate Rust code
    let generated_code = generate_logos_module(&processed_logos);

    // Write to output file
    let mut file = File::create(output_path)?;
    file.write_all(generated_code.as_bytes())?;

    // Tell Cargo to rerun if the source changes
    println!("cargo:rerun-if-changed={}", neofetch_source);

    Ok(())
}

fn extract_logos(content: &str) -> Vec<Logo> {
    let mut logos = Vec::new();

    // Regex pattern to extract logo definitions - no change needed here
    let dist_pattern = Regex::new(
        r#"(?s)"([^"]*)"(\*?)\)\s*set_colors\s+(.*?)read -rd '' ascii_data <<'EOF'(.*?)EOF\s*;;"#,
    )
    .unwrap();

    for cap in dist_pattern.captures_iter(content) {
        let name = cap[1].to_string();
        let is_wildcard = &cap[2] == "*";

        // Parse colors with special handling for "fg"
        let colors: Vec<u8> = cap[3]
            .split_whitespace()
            .map(|s| {
                if s == "fg" {
                    // Treat "fg" as color 7 (light gray/white)
                    // You can choose a different value if preferred
                    7
                } else {
                    s.parse::<u8>().unwrap_or(0)
                }
            })
            .collect();

        // Get ASCII art
        let ascii_art = cap[4].strip_prefix('\n').unwrap_or(&cap[4]).to_string();

        logos.push(Logo {
            name,
            is_wildcard,
            colors,
            ascii_art,
        });
    }

    // Sort logos by name for binary search
    logos.sort_by(|a, b| a.name.cmp(&b.name));

    logos
}

fn process_logos(logos: &[Logo]) -> Vec<ProcessedLogo> {
    logos
        .iter()
        .map(|logo| {
            // Format the ASCII art with ANSI color codes - using only ASCII-safe sequences
            let mut formatted_art = logo.ascii_art.clone();

            // Map for storing color placeholder and its corresponding ANSI code
            let mut color_map = std::collections::HashMap::new();

            // Create color map for all placeholders used in this logo
            for i in 0..6 {
                if i < logo.colors.len() && logo.colors[i] > 0 {
                    let color_value = logo.colors[i];
                    let ansi_code = if color_value <= 7 {
                        // Basic colors (30-37)
                        format!("\x1b[{}m", 30 + color_value)
                    } else {
                        // Extended 256-color mode
                        format!("\x1b[38;5;{}m", color_value)
                    };
                    color_map.insert(format!("${{c{}}}", i + 1), ansi_code);
                }
            }

            // Replace all color placeholders with ANSI codes
            for (placeholder, ansi) in color_map {
                formatted_art = formatted_art.replace(&placeholder, &ansi);
            }

            // Add reset code at the end
            if !formatted_art.ends_with("\x1b[0m") {
                formatted_art.push_str("\x1b[0m");
            }

            // Calculate the maximum visual line length (ignoring color codes)
            let max_line_length = calculate_max_line_length(&logo.ascii_art);

            ProcessedLogo {
                name: logo.name.clone(),
                is_wildcard: logo.is_wildcard,
                ascii_art: formatted_art,
                max_line_length,
            }
        })
        .collect()
}

// Function to calculate the maximum visual line length
fn calculate_max_line_length(ascii_art: &str) -> usize {
    ascii_art
        .lines()
        .map(|line| {
            // Count visible characters by removing color placeholders
            let re = Regex::new(r"\$\{c\d+\}").unwrap();
            let cleaned = re.replace_all(line, "");
            cleaned.chars().count()
        })
        .max()
        .unwrap_or(0)
}

fn generate_logos_module(logos: &[ProcessedLogo]) -> String {
    let mut code = String::new();

    // Add module header
    code.push_str("// Auto-generated code from build script\n\n");

    // Define the Logo struct with max_line_length
    code.push_str("pub struct Logo {\n");
    code.push_str("    pub name: &'static str,\n");
    code.push_str("    pub is_wildcard: bool,\n");
    code.push_str("    pub ascii_art: &'static str,\n");
    code.push_str("    pub max_line_length: usize,\n");
    code.push_str("}\n\n");

    // Start the LOGOS array
    code.push_str("pub static LOGOS: &[Logo] = &[\n");

    // Add each logo definition
    for logo in logos {
        // Create a byte array representation to ensure all characters are properly escaped
        let mut bytes = Vec::new();
        for b in logo.ascii_art.as_bytes() {
            match b {
                b'\x1b' => bytes.extend_from_slice(b"\\x1b"), // Escape character
                b'\\' => bytes.extend_from_slice(b"\\\\"),    // Backslash
                b'"' => bytes.extend_from_slice(b"\\\""),     // Double quote
                b'\n' => bytes.extend_from_slice(b"\\n"),     // Newline
                b'\r' => bytes.extend_from_slice(b"\\r"),     // Carriage return
                b'\t' => bytes.extend_from_slice(b"\\t"),     // Tab
                // For normal printable ASCII characters, just use the character itself
                b' '..=b'~' => bytes.push(*b),
                // For any other character, use Unicode escape
                _ => {
                    let unicode = format!("\\u{{{:04x}}}", *b);
                    bytes.extend_from_slice(unicode.as_bytes());
                }
            }
        }

        // Convert bytes to a string
        let escaped_art = String::from_utf8(bytes).unwrap();

        // Format the Logo instance with max_line_length
        code.push_str(&format!(
            "    Logo {{\n        name: \"{}\",\n        is_wildcard: {},\n        ascii_art: \"{}\",\n",
            logo.name,
            logo.is_wildcard,
            escaped_art
        ));

        code.push_str(&format!(
            "        max_line_length: {},\n    }},\n",
            logo.max_line_length
        ));
    }

    // Close the LOGOS array
    code.push_str("];\n\n");

    // Add utility function to find a logo by name
    code.push_str(
        r#"
pub fn find_logo(distro_name: &str) -> Option<&'static Logo> {
    // First try exact match for non-wildcard logos
    if let Ok(idx) = LOGOS.binary_search_by(|logo| {
        if logo.is_wildcard {
            std::cmp::Ordering::Greater // Skip wildcards for binary search
        } else {
            logo.name.cmp(distro_name)
        }
    }) {
        return Some(&LOGOS[idx]);
    }
    
    // Then try prefix match for wildcard logos
    LOGOS.iter()
        .find(|logo| logo.is_wildcard && distro_name.starts_with(&logo.name))
}
"#,
    );

    code
}
