#![forbid(unsafe_code)]

use chrono::{Datelike, NaiveDate};
use isin::ISIN;
use once_cell::sync::Lazy;
use regex::Regex;
use std::str::FromStr;

/// Structure representing extracted PDF data from Trade Republic documents.
/// 
/// This structure holds all the relevant information needed to generate
/// a standardized filename for Trade Republic PDF documents.
/// 
/// # Security
/// All fields are validated during parsing to ensure safe values:
/// - Date is validated to be within reasonable bounds (2000-current_year+5)
/// - Document type is cleaned and validated
/// - ISIN is validated using proper checksum verification
/// - Asset name is sanitized for safe filesystem usage
#[derive(Debug, PartialEq)]
pub struct PdfData {
    /// Date of the document (validated to be reasonable)
    pub date: NaiveDate,
    /// Type of document (e.g., "Kauf", "Dividende", "Zinsen")
    pub doc_type: String,
    /// ISIN code if present (validated for correctness)
    pub isin: Option<String>,
    /// Asset name (length-validated and sanitized for filename safety)
    pub asset: String,
}

/// Clean up asset names for safe filenames:
/// - Replace forbidden/special chars and whitespace with underscores
/// - Collapse consecutive underscores to one
/// - Trim leading/trailing underscores
/// - Validates input length to prevent excessively long filenames
/// - Removes dangerous characters that could be used for security exploits
pub fn clean_name(name: &str) -> String {
    // Validate input length to prevent potential security issues
    if name.len() > 500 {
        return "Invalid_Asset_Name".to_string();
    }
    
    let mut s = name.to_string();
    
    // Remove control characters and other dangerous Unicode characters
    s.retain(|c| {
        !c.is_control() && 
        c != '\u{202E}' && // Right-to-left override
        c != '\u{202D}' && // Left-to-right override
        c != '\u{200E}' && // Left-to-right mark
        c != '\u{200F}'    // Right-to-left mark
    });
    
    // Use static regex to avoid repeated compilation and potential panics
    use once_cell::sync::Lazy;
    static DANGEROUS_CHARS_RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r#"[<>:"|?*\\./\[\]()]"#).expect("Invalid regex pattern for dangerous chars")
    });
    static WHITESPACE_RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"[\s,]+").expect("Invalid regex pattern for whitespace")
    });
    static MULTIPLE_UNDERSCORES_RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"_+").expect("Invalid regex pattern for underscores")
    });
    
    // Replace dangerous characters and whitespace with underscores
    s = DANGEROUS_CHARS_RE.replace_all(&s, "_").to_string();
    s = WHITESPACE_RE.replace_all(&s, "_").to_string();
    s = MULTIPLE_UNDERSCORES_RE.replace_all(&s, "_").to_string();
    s.trim_matches('_').to_string()
}

/// Main parser: Extracts date, doc_type, ISIN (if present), and asset name from Trade Republic PDF text.
/// Returns None if the text cannot be parsed or contains invalid data.
pub fn parse_pdf_data(text: &str) -> Option<PdfData> {
    // Validate input length to prevent potential DoS attacks
    if text.len() > 1_000_000 {
        return None;
    }
    
    // --- Date extraction with improved error handling ---
    use once_cell::sync::Lazy;
    static DATE_RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"(?i)\b(?:DATUM|DATE)\s*([0-9]{2}\.[0-9]{2}\.[0-9]{4}|[0-9]{4}-[0-9]{2}-[0-9]{2})")
            .expect("Invalid regex pattern for date extraction")
    });
    
    let date_caps = DATE_RE.captures(text)?;
    let date_str = date_caps.get(1)?.as_str();
    
    // Validate and parse date with proper error handling
    let date = if date_str.contains('.') {
        NaiveDate::parse_from_str(date_str, "%d.%m.%Y").ok()?
    } else {
        NaiveDate::parse_from_str(date_str, "%Y-%m-%d").ok()?
    };
    
    // Validate parsed date is reasonable (not too far in past/future)
    let current_year = chrono::Local::now().year();
    if date.year() < 2000 || date.year() > current_year + 5 {
        return None;
    }

    // --- Document type detection (by keyword) ---
    let types = [
        ("WERTPAPIERABRECHNUNG SPARPLAN", "Kauf_Sparplan"),
        ("WERTPAPIERABRECHNUNG SAVEBACK", "Kauf_Saveback"),
        ("WERTPAPIERABRECHNUNG", "Kauf"),
        ("DIVIDENDE", "Dividende"),
        ("ZINSEN", "Zinsen"),
        ("ZINSZAHLUNG", "Zinszahlung"),
        ("Interest Payout", "Zinsen"),
        ("Kapitalmaßnahme", "Kapitalmaßnahme"),
        ("Savings Plan Execution", "Kauf_Sparplan"),
        ("Securities Settlement", "Kauf"),
        ("DEPOTTRANSFER", "Depottransfer"),
        ("DEPOTTRANSFER EINGEGANGEN", "Depottransfer"),
        ("DEPOTAUSZUG", "Depotauszug"),
        ("STEUERLICHE OPTIMIERUNG", "Steuerliche_Optimierung"),
        ("Depotauszug", "Depotauszug"),
        ("Steuerliche Optimierung", "Steuerliche_Optimierung"),
    ];
    // Default type; might get overwritten below (esp. for summary docs)
    let mut doc_type = "Unbekannt".to_string();
    for (needle, replacement) in &types {
        if text.to_uppercase().contains(&needle.to_uppercase()) {
            doc_type = replacement.to_string();
            break;
        }
    }

    // --- ISIN + Asset Extraction (improved, more robust) ---
    let mut isin = None;
    let mut asset = None;
    let lines: Vec<&str> = text.lines().collect();
    
    static ISIN_REGEX: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"\b([A-Z]{2}[A-Z0-9]{9}[0-9])\b")
            .expect("Invalid regex pattern for ISIN extraction")
    });

    for (i, line) in lines.iter().enumerate() {
        if line.contains("Umsatzsteuer") || line.contains("VAT") {
            continue;
        }
        // Look for ISIN *inside* the line (not just if the whole line matches!)
        for caps in ISIN_REGEX.captures_iter(line) {
            let candidate = caps.get(1).map(|m| m.as_str())?;
            if ISIN::from_str(candidate).is_ok() {
                isin = Some(candidate.to_string());

                let mut found_asset = None;

                // If the ISIN is embedded in the line (e.g. "ISIN: ..."), prefer the next non-empty line as asset
                if line.trim() != candidate {
                    // Try next 1-2 lines after the ISIN line
                    for offset in 1..=2 {
                        if let Some(after) = lines.get(i + offset) {
                            let after = after.trim();
                            if !after.is_empty()
                                && !after.contains("ISIN")
                                && !ISIN_REGEX.is_match(after)
                                && after.len() > 3
                                && !after.to_lowercase().contains("gesamt")
                                && !after.to_lowercase().contains("eur")
                                && !after.contains("Stk.")
                                && !after.chars().all(|c| c.is_ascii_digit())
                                && !after.to_lowercase().starts_with("datum")
                                && !after.to_lowercase().starts_with("date")
                            {
                                found_asset = Some(after.to_string());
                                break;
                            }
                        }
                    }
                }
                // If the ISIN is on its own line, search before/after as before
                if found_asset.is_none() {
                    for offset in (1..=3).rev() {
                        if i >= offset {
                            let before = lines[i - offset].trim();
                            if !before.is_empty()
                                && !before.contains("ISIN")
                                && !ISIN_REGEX.is_match(before)
                                && before.len() > 3
                                && !before.chars().all(|c| c.is_ascii_digit())
                                && !before.to_lowercase().contains("gesamt")
                                && !before.to_lowercase().contains("eur")
                                && !before.starts_with("POSITION")
                                && !before.to_lowercase().contains("anzahl")
                                && !before.contains("Stk.")
                                && !before.to_lowercase().starts_with("datum")
                                && !before.to_lowercase().starts_with("date")
                            {
                                found_asset = Some(before.to_string());
                                break;
                            }
                        }
                    }
                }
                if found_asset.is_none() {
                    found_asset = Some(candidate.to_string());
                }
                asset = found_asset;
                break;
            }
        }
        if isin.is_some() {
            break;
        }
    }

    // --- Special handling for Sammelbelege (summary docs with both Zinsen and Dividende) ---
    let mut found_zinsen = false;
    let mut found_dividende = false;
    let mut assets_vec = vec![];

    for line in text.lines() {
        let lower = line.to_lowercase();
        if lower.contains("cash zinsen") && !found_zinsen {
            found_zinsen = true;
            assets_vec.push("Guthaben_Zinsen".to_string());
        }
        if lower.contains("geldmarkt dividende") && !found_dividende {
            found_dividende = true;
            assets_vec.push("Geldmarkt_Dividende".to_string());
        }
    }

    if isin.is_none() && asset.is_none() && (found_zinsen || found_dividende) {
        doc_type = match (found_zinsen, found_dividende) {
            (true, true) => "Zinsen_und_Dividende".to_string(),
            (true, false) => "Zinsen".to_string(),
            (false, true) => "Dividende".to_string(),
            _ => "Unbekannt".to_string(),
        };
        asset = Some(assets_vec.join("_und_"));
    }

    // --- Fallbacks für andere Fälle ---
    if asset.is_none() {
        static POSITION_RE: Lazy<Regex> = Lazy::new(|| {
            Regex::new(r"POSITION[^\n]*\n([^\n]+)")
                .expect("Invalid regex pattern for position extraction")
        });
        
        // Extract position info if available
        if let Some(caps) = POSITION_RE.captures(text)
            && let Some(position_match) = caps.get(1) {
            asset = Some(position_match.as_str().trim().to_string());
        }
        
        // Special handling for Zinsen/Dividende without position
        if asset.is_none() && (doc_type == "Zinsen" || doc_type == "Zinszahlung" || doc_type == "Dividende") && isin.is_none() {
            asset = Some("Guthaben".to_string());
        }
        
        // Final fallback
        if asset.is_none() {
            asset = Some("Guthaben".to_string());
        }
    }

    // Spezialfälle für bestimmte Dokumenttypen
    if doc_type == "Depottransfer" {
        static TRANSFER_RE: Lazy<Regex> = Lazy::new(|| {
            Regex::new(r"Depottransfer eingegangen\s+(.+?)(?:\n|$)")
                .expect("Invalid regex pattern for transfer extraction")
        });
        if let Some(caps) = TRANSFER_RE.captures(text)
            && let Some(transfer_match) = caps.get(1) {
            asset = Some(transfer_match.as_str().trim().to_string());
        }
    }
    if doc_type == "Depotauszug" {
        asset = Some("Depot".to_string());
    }
    if doc_type == "Steuerliche_Optimierung" {
        asset = Some("Steuer".to_string());
    }

    // Validate final asset name but don't clean it yet
    let final_asset = asset.unwrap_or_else(|| "Guthaben".to_string());
    
    // Ensure asset name is not empty and not excessively long
    let validated_asset = if final_asset.trim().is_empty() || final_asset.len() > 500 {
        "Guthaben".to_string()
    } else {
        final_asset
    };

    Some(PdfData {
        date,
        doc_type,
        isin,
        asset: validated_asset,
    })
}

/// Builds the filename: date, type, ISIN (if present), asset name (cleaned)
/// Validates all components to ensure safe filesystem operations
pub fn build_filename(pdf_data: &PdfData, orig_name: &str) -> String {
    let date = pdf_data.date.format("%Y_%m_%d").to_string();
    
    // Clean and validate document type
    let doc_type = clean_name(&pdf_data.doc_type.replace(' ', "_"));
    
    // Clean and validate asset name with length limit
    let mut namepart = clean_name(&pdf_data.asset);
    if namepart.len() > 50 {
        namepart.truncate(50);
        namepart = namepart.trim_end_matches('_').to_string();
    }
    
    // Validate ISIN if present
    let isin_part = pdf_data
        .isin
        .as_ref()
        .filter(|isin| isin.len() == 12 && isin.chars().all(|c| c.is_ascii_alphanumeric()))
        .map(|s| format!("_{}", s))
        .unwrap_or_default();
    
    // Validate and clean file extension
    let ext = std::path::Path::new(orig_name)
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .filter(|ext| ext.len() <= 10 && ext.chars().all(|c| c.is_ascii_alphanumeric()))
        .unwrap_or("pdf");
    
    format!("{}_{}{}_{}.{}", date, doc_type, isin_part, namepart, ext)
}
