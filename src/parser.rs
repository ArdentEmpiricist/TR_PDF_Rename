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
        for caps in possible_isin_regex.captures_iter(line) {
            let candidate = caps.get(1).unwrap().as_str();
            if ISIN::from_str(candidate).is_ok() {
                isin = Some(candidate.to_string());
                // NEW: asset detection
                let mut found_asset = None;
                for offset in 1..=2 {
                    if let Some(after_line) = lines.get(i + offset) {
                        let after_line = after_line.trim();
                        if !after_line.is_empty()
                            && !after_line.contains("ISIN")
                            && !possible_isin_regex.is_match(after_line)
                            && after_line.len() > 3
                        {
                            found_asset = Some(after_line.to_string());
                            break;
                        }
                    }
                }
                if found_asset.is_none() && i >= 1 {
                    let before_line = lines[i - 1].trim();
                    if !before_line.is_empty()
                        && !before_line.contains("ISIN")
                        && !possible_isin_regex.is_match(before_line)
                        && before_line.len() > 3
                    {
                        found_asset = Some(before_line.to_string());
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
        if (doc_type == "Zinsen" || doc_type == "Zinszahlung") && isin.is_none() {
            let pos_re = Regex::new(r"POSITION[^\n]*\n([^\n]+)").unwrap();
            if let Some(caps) = pos_re.captures(text) {
                asset = Some(caps.get(1)?.as_str().trim().to_string());
            } else {
                asset = Some("Guthaben".to_string());
            }
        } else if doc_type == "Dividende" && isin.is_none() {
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
        .map(|s| format!("_{}", s))
        .unwrap_or_default();
    let ext = std::path::Path::new(orig_name)
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or("pdf");
    format!("{}_{}{}_{}.{}", date, doc_type, isin_part, namepart, ext)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn test_clean_name_removes_special_chars() {
        assert_eq!(clean_name("MSCI World USD (Dist)"), "MSCI_World_USD_Dist");
        assert_eq!(clean_name("Test   (ETF) / Name."), "Test_ETF_Name");
        assert_eq!(
            clean_name("  _Multiple__underscores__  "),
            "Multiple_underscores"
        );
        assert_eq!(clean_name("Leading_"), "Leading");
        assert_eq!(clean_name("Trailing_"), "Trailing");
        assert_eq!(clean_name("____Test___Name___"), "Test_Name");
    }

    #[test]
    fn test_isin_and_asset_extraction() {
        let input = "DATUM 31.07.2025\nISIN: IE00BZ163G84\nEUR Corporate Bond (Dist)\n";
        let result = parse_pdf_data(input).unwrap();
        assert_eq!(result.isin.as_deref(), Some("IE00BZ163G84"));
        assert_eq!(result.asset, "EUR Corporate Bond (Dist)");
    }

    #[test]
    fn test_isin_name_across_lines() {
        let input = "DATUM 30.07.2025\nISIN:\nIE00BF4RFH31\nMSCI World Small Cap USD (Acc)\n";
        let result = parse_pdf_data(input).unwrap();
        assert_eq!(result.isin.as_deref(), Some("IE00BF4RFH31"));
        assert_eq!(result.asset, "MSCI World Small Cap USD (Acc)");
    }

    #[test]
    fn test_dividende_with_position() {
        let input = "DATUM 15.07.2024\nDIVIDENDE\nPOSITION\niShares Core DAX UCITS ETF\n";
        let result = parse_pdf_data(input).unwrap();
        assert_eq!(result.doc_type, "Dividende");
        assert_eq!(result.asset, "iShares Core DAX UCITS ETF");
    }

    #[test]
    fn test_zinsen_with_position() {
        let input = "DATUM 01.08.2025\nZINSEN\nPOSITION\nSonderzinszahlung 5% Anleihe 2029\n";
        let result = parse_pdf_data(input).unwrap();
        assert_eq!(result.doc_type, "Zinsen");
        assert_eq!(result.asset, "Sonderzinszahlung 5% Anleihe 2029");
    }

    #[test]
    fn test_zinsen_no_position_sets_guthaben() {
        let input = "DATUM 01.08.2025\nZINSEN\n";
        let result = parse_pdf_data(input).unwrap();
        assert_eq!(result.doc_type, "Zinsen");
        assert_eq!(result.asset, "Guthaben");
    }

    #[test]
    fn test_dividende_no_position_sets_guthaben() {
        let input = "DATUM 01.08.2025\nDIVIDENDE\n";
        let result = parse_pdf_data(input).unwrap();
        assert_eq!(result.doc_type, "Dividende");
        assert_eq!(result.asset, "Guthaben");
    }

    #[test]
    fn test_summary_document_zinsen_und_geldmarkt_dividende() {
        let input = r#"
            DATUM 01.08.2025
            Interest Payout
            Cash Zinsen 2,00% 01.07.2025 - 31.07.2025 18,44 EUR
            Geldmarkt Dividende 2,00% 01.07.2025 - 31.07.2025 68,15 EUR
        "#;
        let result = parse_pdf_data(input).unwrap();
        assert_eq!(result.doc_type, "Zinsen_und_Dividende");
        assert_eq!(result.asset, "Guthaben_Zinsen_und_Geldmarkt_Dividende");
    }

    #[test]
    fn test_summary_document_only_zinsen() {
        let input = r#"
            DATUM 01.08.2025
            Interest Payout
            Cash Zinsen 1,50% 01.07.2025 - 31.07.2025 18,44 EUR
        "#;
        let result = parse_pdf_data(input).unwrap();
        assert_eq!(result.doc_type, "Zinsen");
        assert_eq!(result.asset, "Guthaben_Zinsen");
    }

    #[test]
    fn test_summary_document_only_dividende() {
        let input = r#"
            DATUM 01.08.2025
            Interest Payout
            Geldmarkt Dividende 1,00% 01.07.2025 - 31.07.2025 99,99 EUR
        "#;
        let result = parse_pdf_data(input).unwrap();
        assert_eq!(result.doc_type, "Dividende");
        assert_eq!(result.asset, "Geldmarkt_Dividende");
    }

    #[test]
    fn test_no_isin_and_no_asset_fallback_depot() {
        let input = "DATUM 31.07.2025\nDEPOTAUSZUG\n";
        let result = parse_pdf_data(input).unwrap();
        assert_eq!(result.doc_type, "Depotauszug");
        assert_eq!(result.asset, "Depot");
    }

    #[test]
    fn test_steuerliche_optimierung_sets_asset_steuer() {
        let input = "DATUM 31.07.2025\nSTEUERLICHE OPTIMIERUNG\n";
        let result = parse_pdf_data(input).unwrap();
        assert_eq!(result.doc_type, "Steuerliche_Optimierung");
        assert_eq!(result.asset, "Steuer");
    }

    #[test]
    fn test_umsatzsteuer_id_is_not_isin() {
        let input = "DATUM 31.07.2025\nUmsatzsteuer-ID: DE307510626\nName: Thomas Pischke\n";
        let result = parse_pdf_data(input).unwrap();
        // No ISIN should be found here
        assert_eq!(result.isin, None);
    }

    #[test]
    fn test_build_filename_has_no_double_underscores() {
        let pdf_data = PdfData {
            date: NaiveDate::from_ymd(2025, 7, 2),
            doc_type: "Kauf_Sparplan".to_string(),
            isin: Some("IE00BK1PV551".to_string()),
            asset: "MSCI World USD (Dist)".to_string(),
        };
        let name = build_filename(&pdf_data, "orig.pdf");
        // Should NOT contain "__" nor end with "_"
        assert!(!name.contains("__"));
        assert!(!name.ends_with('_'));
        assert!(name.contains("MSCI_World_USD_Dist"));
    }
}
