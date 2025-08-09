#![forbid(unsafe_code)]

use chrono::NaiveDate;
use isin::ISIN;
use regex::Regex;
use std::str::FromStr;

/// Structure representing extracted PDF data.
#[derive(Debug, PartialEq)]
pub struct PdfData {
    pub date: NaiveDate,
    pub doc_type: String,
    pub isin: Option<String>,
    pub asset: String,
}

/// Clean up asset names for safe filenames:
/// - Replace forbidden/special chars and whitespace with underscores
/// - Collapse consecutive underscores to one
/// - Trim leading/trailing underscores
pub fn clean_name(name: &str) -> String {
    let mut s = name.to_string();
    s = Regex::new(r"[ /()\.,\[\]]+")
        .unwrap()
        .replace_all(&s, "_")
        .to_string();
    s = Regex::new(r"_+").unwrap().replace_all(&s, "_").to_string();
    s.trim_matches('_').to_string()
}

/// Main parser: Extracts date, doc_type, ISIN (if present), and asset name from Trade Republic PDF text.
pub fn parse_pdf_data(text: &str) -> Option<PdfData> {
    // --- Date extraction ---
    let date_re = Regex::new(
        r"(?i)\b(?:DATUM|DATE)\s*([0-9]{2}\.[0-9]{2}\.[0-9]{4}|[0-9]{4}-[0-9]{2}-[0-9]{2})",
    )
    .unwrap();
    let date_caps = date_re.captures(text)?;
    let date_str = date_caps.get(1)?.as_str();
    let date = if date_str.contains('.') {
        NaiveDate::parse_from_str(date_str, "%d.%m.%Y").ok()?
    } else {
        NaiveDate::parse_from_str(date_str, "%Y-%m-%d").ok()?
    };

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
    let possible_isin_regex = Regex::new(r"\b([A-Z]{2}[A-Z0-9]{9}[0-9])\b").unwrap();

    for (i, line) in lines.iter().enumerate() {
        if line.contains("Umsatzsteuer") || line.contains("VAT") {
            continue;
        }
        // Look for ISIN *inside* the line (not just if the whole line matches!)
        for caps in possible_isin_regex.captures_iter(line) {
            let candidate = caps.get(1).unwrap().as_str();
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
                                && !possible_isin_regex.is_match(after)
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
                                && !possible_isin_regex.is_match(before)
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
        if (doc_type == "Zinsen" || doc_type == "Zinszahlung" || doc_type == "Dividende")
            && isin.is_none()
        {
            let pos_re = Regex::new(r"POSITION[^\n]*\n([^\n]+)").unwrap();
            if let Some(caps) = pos_re.captures(text) {
                asset = Some(caps.get(1)?.as_str().trim().to_string());
            } else {
                asset = Some("Guthaben".to_string());
            }
        } else {
            let pos_re = Regex::new(r"POSITION[^\n]*\n([^\n]+)").unwrap();
            if let Some(caps) = pos_re.captures(text) {
                asset = Some(caps.get(1).unwrap().as_str().trim().to_string());
            } else {
                asset = Some("Guthaben".to_string());
            }
        }
    }

    // Spezialfälle für bestimmte Dokumenttypen
    if doc_type == "Depottransfer" {
        let transfer_re = Regex::new(r"Depottransfer eingegangen\s+(.+?)(?:\n|$)").unwrap();
        if let Some(caps) = transfer_re.captures(text) {
            asset = Some(caps.get(1).unwrap().as_str().trim().to_string());
        }
    }
    if doc_type == "Depotauszug" {
        asset = Some("Depot".to_string());
    }
    if doc_type == "Steuerliche_Optimierung" {
        asset = Some("Steuer".to_string());
    }

    Some(PdfData {
        date,
        doc_type,
        isin,
        asset: asset.unwrap_or_else(|| "Guthaben".to_string()),
    })
}

/// Builds the filename: date, type, ISIN (if present), asset name (cleaned)
pub fn build_filename(pdf_data: &PdfData, orig_name: &str) -> String {
    let date = pdf_data.date.format("%Y_%m_%d").to_string();
    let doc_type = pdf_data.doc_type.replace(' ', "_");
    let mut namepart = clean_name(&pdf_data.asset);
    if namepart.len() > 50 {
        namepart.truncate(50);
    }
    let isin_part = pdf_data
        .isin
        .as_ref()
        .map(|s| format!("_{s}"))
        .unwrap_or_default();
    let ext = std::path::Path::new(orig_name)
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or("pdf");
    format!("{date}_{doc_type}{isin_part}_{namepart}.{ext}")
}
