#![forbid(unsafe_code)]

use chrono::{Datelike, NaiveDate};
use isin::ISIN;
use regex::Regex;
use std::str::FromStr;
use std::sync::LazyLock;

// Pre-compiled regex patterns for performance and security
static DANGEROUS_CHARS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"[<>:"|?*\\./\[\]()]"#).expect("Invalid regex pattern for dangerous chars")
});
static WHITESPACE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[\s,]+").expect("Invalid regex pattern for whitespace"));
static MULTIPLE_UNDERSCORES_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"_+").expect("Invalid regex pattern for underscores"));
static DATE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:DATUM|DATE|ERSTELLT\s+AM|STAND|GENERATED|CREATED|AS\s+OF)\s*[:\-]?\s*([0-9]{2}\.[0-9]{2}\.[0-9]{4}|[0-9]{4}-[0-9]{2}-[0-9]{2})",
    )
    .expect("Invalid regex pattern for date extraction")
});
static ANY_DATE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b([0-9]{2}\.[0-9]{2}\.[0-9]{4}|[0-9]{4}-[0-9]{2}-[0-9]{2})\b")
        .expect("Invalid regex pattern for fallback date extraction")
});
static TEXTUAL_DATUM_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\bDATUM[^\n]*?([0-3]?\d)\s+([[:alpha:].]+)\s+(20[0-9]{2})")
        .expect("Invalid regex pattern for textual DATUM extraction")
});
static TEXTUAL_DATE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b([0-3]?\d)\s+([[:alpha:].]+)\s+(20[0-9]{2})\b")
        .expect("Invalid regex pattern for textual date extraction")
});
static TEXTUAL_RANGE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\bDATUM[^\n]*?([0-3]?\d)\s+([[:alpha:].]+)\s+(20[0-9]{2})\s*[-\u{2013}\u{2014}]\s*([0-3]?\d)\s+([[:alpha:].]+)\s+(20[0-9]{2})",
    )
    .expect("Invalid regex pattern for textual date range extraction")
});
static NUMERIC_RANGE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:DATUM|DATE|ERSTELLT\s+AM|STAND|GENERATED|CREATED|AS\s+OF)\s*[:\-]?\s*([0-9]{2}\.[0-9]{2}\.[0-9]{4}|[0-9]{4}-[0-9]{2}-[0-9]{2})\s*[-\u{2013}\u{2014}]\s*([0-9]{2}\.[0-9]{2}\.[0-9]{4}|[0-9]{4}-[0-9]{2}-[0-9]{2})",
    )
    .expect("Invalid regex pattern for numeric date range extraction")
});
static ISIN_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b([A-Z]{2}[A-Z0-9]{9}[0-9])\b")
        .expect("Invalid regex pattern for ISIN extraction")
});
static IBAN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\bIBAN\b[:\s]*([A-Z]{2}[A-Z0-9\s]{8,40})")
        .expect("Invalid regex pattern for IBAN extraction")
});
static POSITION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"POSITION[^\n]*\n([^\n]+)").expect("Invalid regex pattern for position extraction")
});
static TRANSFER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"Depottransfer eingegangen\s+(.+?)(?:\n|$)")
        .expect("Invalid regex pattern for transfer extraction")
});
static YEAR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(20[0-9]{2})\b").expect("Invalid regex pattern for year extraction")
});

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
        !c.is_control()
            && c != '\u{202E}' // Right-to-left override
            && c != '\u{202D}' // Left-to-right override
            && c != '\u{200E}' // Left-to-right mark
            && c != '\u{200F}' // Right-to-left mark
    });

    // Replace dangerous characters and whitespace with underscores
    s = DANGEROUS_CHARS_RE.replace_all(&s, "_").to_string();
    s = WHITESPACE_RE.replace_all(&s, "_").to_string();
    s = MULTIPLE_UNDERSCORES_RE.replace_all(&s, "_").to_string();
    s.trim_matches('_').to_string()
}

/// Main parser: Extracts date, `doc_type`, ISIN (if present), and asset name from Trade Republic PDF text.
/// Returns None if the text cannot be parsed or contains invalid data.
#[allow(clippy::too_many_lines)]
pub fn parse_pdf_data(text: &str) -> Option<PdfData> {
    // Validate input length to prevent potential DoS attacks
    if text.len() > 1_000_000 {
        return None;
    }

    // --- Date extraction with improved error handling ---
    let date = extract_date(text)?;

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
        (
            "KOSTENINFORMATION ZUM SAVE-BACK",
            "Kosteninformation_Saveback",
        ),
        ("KOSTENINFORMATION ZUM SAVE", "Kosteninformation_Saveback"),
        ("EX-POST KOSTENINFORMATION", "Ex_Post_Kosteninformation"),
        ("JAHRESSTEUERBESCHEINIGUNG", "Jahressteuerbescheinigung"),
        ("STEUERREPORT", "Steuerreport"),
        ("KONTOAUSZUG", "Kontoauszug"),
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
    let text_upper = text.to_uppercase();
    for (needle, replacement) in &types {
        if text_upper.contains(&needle.to_uppercase()) {
            doc_type = (*replacement).to_string();
            break;
        }
    }

    // --- ISIN + Asset Extraction (improved, more robust) ---
    let mut isin = None;
    let mut asset = None;
    let lines: Vec<&str> = text.lines().collect();

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
        // Extract position info if available
        if let Some(caps) = POSITION_RE.captures(text)
            && let Some(position_match) = caps.get(1)
        {
            asset = Some(position_match.as_str().trim().to_string());
        }

        // Special handling for Zinsen/Dividende without position
        if asset.is_none()
            && (doc_type == "Zinsen" || doc_type == "Zinszahlung" || doc_type == "Dividende")
            && isin.is_none()
        {
            asset = Some("Guthaben".to_string());
        }

        // Final fallback
        if asset.is_none() {
            asset = Some("Guthaben".to_string());
        }
    }

    // Spezialfälle für bestimmte Dokumenttypen
    if doc_type == "Depottransfer"
        && let Some(caps) = TRANSFER_RE.captures(text)
        && let Some(transfer_match) = caps.get(1)
    {
        asset = Some(transfer_match.as_str().trim().to_string());
    }
    if doc_type == "Depotauszug" {
        asset = Some("Depot".to_string());
    }
    if doc_type == "Steuerliche_Optimierung" {
        asset = Some("Steuer".to_string());
    }
    if doc_type == "Kontoauszug" {
        isin = None;
        asset = extract_iban(text).or_else(|| Some("Konto".to_string()));
    }
    if doc_type == "Kosteninformation_Saveback"
        || doc_type == "Ex_Post_Kosteninformation"
        || doc_type == "Jahressteuerbescheinigung"
        || doc_type == "Steuerreport"
    {
        let referenced_year = YEAR_RE
            .captures_iter(text)
            .filter_map(|caps| caps.get(1))
            .filter_map(|m| m.as_str().parse::<i32>().ok())
            .filter(|year| *year >= 2000 && *year <= date.year())
            .fold(None, |acc, year| {
                if year == date.year() - 1 {
                    Some(year)
                } else {
                    match acc {
                        Some(existing) => Some(existing.max(year)),
                        None => Some(year),
                    }
                }
            })
            .or_else(|| {
                let prev = date.year() - 1;
                if prev >= 2000 { Some(prev) } else { None }
            })
            .unwrap_or_else(|| date.year());
        asset = Some(referenced_year.to_string());
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
        .map(|s| format!("_{s}"))
        .unwrap_or_default();

    // Validate and clean file extension
    let ext = std::path::Path::new(orig_name)
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .filter(|ext| ext.len() <= 10 && ext.chars().all(|c| c.is_ascii_alphanumeric()))
        .unwrap_or("pdf");

    format!("{date}_{doc_type}{isin_part}_{namepart}.{ext}")
}

fn extract_date(text: &str) -> Option<NaiveDate> {
    if let Some(caps) = TEXTUAL_RANGE_RE.captures(text) {
        return parse_textual_date_components(
            caps.get(4)?.as_str(),
            caps.get(5)?.as_str(),
            caps.get(6)?.as_str(),
        );
    }

    if let Some(caps) = NUMERIC_RANGE_RE.captures(text) {
        return parse_numeric_date_component(caps.get(2)?.as_str());
    }

    if let Some(date) = TEXTUAL_DATUM_RE
        .captures_iter(text)
        .filter_map(|caps| {
            parse_textual_date_components(
                caps.get(1)?.as_str(),
                caps.get(2)?.as_str(),
                caps.get(3)?.as_str(),
            )
        })
        .next()
    {
        return Some(date);
    }

    if let Some(date) = DATE_RE
        .captures_iter(text)
        .filter_map(|caps| caps.get(1))
        .filter_map(|m| parse_numeric_date(m.as_str()))
        .next()
    {
        return Some(date);
    }

    if let Some(date) = ANY_DATE_RE
        .captures_iter(text)
        .filter_map(|caps| caps.get(1))
        .filter_map(|m| parse_numeric_date(m.as_str()))
        .next()
    {
        return Some(date);
    }

    if let Some(date) = TEXTUAL_DATE_RE
        .captures_iter(text)
        .filter_map(|caps| {
            parse_textual_date_components(
                caps.get(1)?.as_str(),
                caps.get(2)?.as_str(),
                caps.get(3)?.as_str(),
            )
        })
        .next()
    {
        return Some(date);
    }

    None
}

fn parse_numeric_date(date_str: &str) -> Option<NaiveDate> {
    if let Some(date) = parse_numeric_date_component(date_str) {
        return Some(date);
    }

    for separator in [" - ", " \u{2013} ", " \u{2014} "] {
        if let Some(pos) = date_str.find(separator) {
            let tail = &date_str[pos + separator.len()..];
            if let Some(date) = parse_numeric_date_component(tail) {
                return Some(date);
            }
        }
    }

    None
}

fn parse_numeric_date_component(date_str: &str) -> Option<NaiveDate> {
    let trimmed = date_str.trim();

    if trimmed.contains('.') {
        NaiveDate::parse_from_str(trimmed, "%d.%m.%Y").ok()
    } else if trimmed.len() == 10
        && trimmed.as_bytes().get(4) == Some(&b'-')
        && trimmed.as_bytes().get(7) == Some(&b'-')
    {
        NaiveDate::parse_from_str(trimmed, "%Y-%m-%d").ok()
    } else {
        None
    }
}

fn parse_textual_date_components(day: &str, month: &str, year: &str) -> Option<NaiveDate> {
    let day: u32 = day.trim().parse().ok()?;
    let year: i32 = year.trim().parse().ok()?;
    let month = month_name_to_number(month)?;
    NaiveDate::from_ymd_opt(year, month, day)
}

fn month_name_to_number(month_raw: &str) -> Option<u32> {
    let mut month = month_raw.trim().to_lowercase();
    month = month.replace('.', "");
    month = month.replace('\u{00E4}', "ae");
    month = month.replace('\u{00F6}', "oe");
    month = month.replace('\u{00FC}', "ue");
    month = month.replace('\u{00DF}', "ss");
    month.retain(|c| !c.is_whitespace());

    match month.as_str() {
        "jan" | "januar" | "january" => Some(1),
        "feb" | "februar" | "february" => Some(2),
        "mar" | "march" | "maerz" | "marz" => Some(3),
        "apr" | "april" => Some(4),
        "mai" | "may" => Some(5),
        "jun" | "juni" | "june" => Some(6),
        "jul" | "juli" | "july" => Some(7),
        "aug" | "august" => Some(8),
        "sep" | "sept" | "september" => Some(9),
        "okt" | "oktober" | "oct" | "october" => Some(10),
        "nov" | "november" => Some(11),
        "dez" | "dezember" | "dec" | "december" => Some(12),
        _ => None,
    }
}

fn extract_iban(text: &str) -> Option<String> {
    let caps = IBAN_RE.captures(text)?;
    let raw = caps.get(1)?.as_str();
    let cleaned: String = raw.chars().filter(|c| !c.is_whitespace()).collect();

    if cleaned.len() < 15 {
        return None;
    }

    if !cleaned.chars().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }

    let upper = cleaned.to_uppercase();
    let max_len = upper.len().min(34);

    for len in (15..=max_len).rev() {
        let candidate = &upper[..len];
        if is_valid_iban(candidate) {
            return Some(candidate.to_string());
        }
    }

    None
}

fn is_valid_iban(iban: &str) -> bool {
    let len = iban.len();
    if len < 15 || len > 34 {
        return false;
    }

    let bytes = iban.as_bytes();
    if !bytes.get(0).map_or(false, u8::is_ascii_alphabetic)
        || !bytes.get(1).map_or(false, u8::is_ascii_alphabetic)
        || !bytes.get(2).map_or(false, u8::is_ascii_digit)
        || !bytes.get(3).map_or(false, u8::is_ascii_digit)
    {
        return false;
    }

    if !iban.chars().all(|c| c.is_ascii_alphanumeric()) {
        return false;
    }

    let rearranged = format!("{}{}", &iban[4..], &iban[..4]);
    let mut remainder: u32 = 0;

    for ch in rearranged.chars() {
        if let Some(digit) = ch.to_digit(10) {
            remainder = (remainder * 10 + digit) % 97;
        } else {
            let value = (ch as u8 - b'A' + 10) as u32;
            remainder = (remainder * 10 + value / 10) % 97;
            remainder = (remainder * 10 + value % 10) % 97;
        }
    }

    remainder == 1
}
