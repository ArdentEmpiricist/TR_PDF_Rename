use chrono::NaiveDate;
use isin::ISIN;
use regex::Regex;
use std::str::FromStr;

/// Holds structured PDF data after parsing (date, type, ISIN, asset name)
#[derive(Debug, PartialEq)]
pub struct PdfData {
    pub date: NaiveDate,
    pub doc_type: String,
    pub isin: Option<String>,
    pub asset: String,
}

/// Parses text extracted from a Trade Republic PDF and returns key structured info.
///
/// - Finds the date (supports both German and ISO format)
/// - Detects the document type by keyword
/// - Finds the ISIN (using the `isin` crate for correct validation! Skips VAT IDs, etc.)
/// - Associates a robust asset name with the ISIN (checks surrounding lines, etc.)
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
    let mut doc_type = "Unbekannt".to_string();
    for (needle, replacement) in &types {
        if text.to_uppercase().contains(&needle.to_uppercase()) {
            doc_type = replacement.to_string();
            break;
        }
    }

    // --- ISIN + Asset Extraction (with real ISIN validation via `isin` crate) ---
    let mut isin = None;
    let mut asset = None;
    let lines: Vec<&str> = text.lines().collect();
    let possible_isin_regex = Regex::new(r"\b([A-Z]{2}[A-Z0-9]{9}[0-9])\b").unwrap(); // ISIN: 2 letters, 9 alnum, 1 digit

    for (i, line) in lines.iter().enumerate() {
        // Only try ISINs in lines that do NOT contain "Umsatzsteuer" or "VAT" (context filter)
        if line.contains("Umsatzsteuer") || line.contains("VAT") {
            continue;
        }
        // Look for ISIN-like substrings in this line
        for caps in possible_isin_regex.captures_iter(line) {
            let candidate = caps.get(1).unwrap().as_str();
            if ISIN::from_str(candidate).is_ok() {
                // Valid ISIN! (Luhn check, syntax check via crate)
                isin = Some(candidate.to_string());
                // Try to extract the asset name from nearby lines (up to 2 before/after)
                let mut name_candidate = None;
                for offset in [-2, -1, 1, 2].iter() {
                    let idx = i.checked_add_signed(*offset);
                    if let Some(idx) = idx {
                        if idx < lines.len() {
                            let l = lines[idx].trim();
                            // Asset name: Not empty, not ISIN, not "Stk", not "Shares", not VAT, not just numbers
                            if !l.is_empty()
                                && !l.contains("ISIN")
                                && !l.contains("Stk")
                                && !l.contains("Shares")
                                && !l.contains("Kapitalertrag")
                                && !l.contains("VAT")
                                && !l.contains("Umsatzsteuer")
                                && !l.chars().all(|c| c.is_numeric())
                                && !l.chars().all(|c| c == '.')
                            {
                                // Exclude lines that are only numbers or ISINs
                                if !Regex::new(r"^\d+[.,]?\d* ?(EUR|USD)?$")
                                    .unwrap()
                                    .is_match(l)
                                    && !possible_isin_regex.is_match(l)
                                {
                                    name_candidate = Some(l.to_string());
                                    break;
                                }
                            }
                        }
                    }
                }
                // If no candidate, try the ISIN line itself (if not only ISIN)
                if name_candidate.is_none() {
                    let l = lines[i].trim();
                    if !l.contains("ISIN") && l.len() > 6 && !possible_isin_regex.is_match(l) {
                        name_candidate = Some(l.to_string());
                    }
                }
                // If nothing found, fallback to ISIN
                asset = name_candidate.or(Some(candidate.to_string()));
                break;
            }
        }
        if isin.is_some() {
            break;
        }
    }

    // Fallback: Use POSITIONS (dividends, Zinsen, etc.)
    if asset.is_none() {
        let pos_re = Regex::new(r"POSITION[^\n]*\n([^\n]+)").unwrap();
        if let Some(caps) = pos_re.captures(text) {
            asset = Some(
                caps.get(1)
                    .unwrap()
                    .as_str()
                    .split_whitespace()
                    .take(4)
                    .collect::<Vec<_>>()
                    .join(" "),
            );
        } else {
            asset = Some("Cash".to_string());
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
    if doc_type == "Zinszahlung" && isin.is_none() {
        let pos_re = Regex::new(r"POSITION[^\n]*\n([^\n]+)").unwrap();
        if let Some(caps) = pos_re.captures(text) {
            asset = Some(caps.get(1)?.as_str().trim().to_string());
        }
    }

    Some(PdfData {
        date,
        doc_type,
        isin,
        asset: asset.unwrap_or_else(|| "Cash".to_string()),
    })
}

fn clean_name(name: &str) -> String {
    let mut s = name.to_string();
    // Replace unwanted chars by underscore
    s = Regex::new(r"[ /()\.,\[\]]+")
        .unwrap()
        .replace_all(&s, "_")
        .to_string();
    // Replace multiple consecutive underscores by one
    s = Regex::new(r"_+").unwrap().replace_all(&s, "_").to_string();
    // Remove leading/trailing underscores
    s.trim_matches('_').to_string()
}

/// Builds a safe filename: date, doc_type, ISIN (if present), and asset name
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
        .map(|s| format!("_{}", s))
        .unwrap_or_default();
    let ext = std::path::Path::new(orig_name)
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or("pdf");
    format!("{}_{}{}_{}.{}", date, doc_type, isin_part, namepart, ext)
}
