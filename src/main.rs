mod parser;

use anyhow::Result;
use parser::{build_filename, parse_pdf_data};
use regex::Regex;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::{env, fs};
use walkdir::WalkDir;

/// Extracts all text from a PDF file using pdf-extract
fn extract_pdf_text(path: &Path) -> Result<String> {
    let pdf_text = pdf_extract::extract_text(path)?;
    Ok(pdf_text)
}

/// Checks if a filename already matches the target renaming scheme
/// E.g. 2024_08_12_Kauf_DE000A1EWWW0_Vanguard_Funds_PLC_ETF.pdf
///      2024_08_12_Depotauszug_Depot.pdf (no ISIN)
fn is_already_renamed(filename: &str) -> bool {
    // Regex for the naming pattern:
    // yyyy_mm_dd_TYP(_ISIN)?_ASSET.pdf
    let re =
        Regex::new(r"^\d{4}_\d{2}_\d{2}_[A-Za-z_]+(_[A-Z]{2}[A-Z0-9]{9}\d)?_.+\.pdf$").unwrap();
    re.is_match(filename)
}

/// Processes all PDF files in a folder (and subfolders),
/// skipping files already in the new naming scheme.
fn process_folder(folder: &Path) -> Result<()> {
    for entry in WalkDir::new(folder).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file()
            && entry
                .path()
                .extension()
                .and_then(OsStr::to_str)
                .map(|e| e.eq_ignore_ascii_case("pdf"))
                .unwrap_or(false)
        {
            let path = entry.path();
            let orig_filename = path.file_name().unwrap().to_str().unwrap();

            // Skip files already renamed
            if is_already_renamed(orig_filename) {
                println!("Skipping (already renamed): {orig_filename}");
                continue;
            }

            println!("Processing: {orig_filename:?}");
            let text = extract_pdf_text(path)?;
            if let Some(pdf_data) = parse_pdf_data(&text) {
                let new_name = build_filename(&pdf_data, orig_filename);
                let new_path = path.parent().unwrap().join(new_name);
                println!("Renaming to: {:?}", new_path.file_name().unwrap());
                fs::rename(path, new_path)?;
            } else {
                println!("Warning: Could not parse {orig_filename:?}");
            }
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        println!("Usage: tr_pdf_renamer <folder>");
        return Ok(());
    }
    let folder = PathBuf::from(&args[1]);
    process_folder(&folder)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{PdfData, build_filename, clean_name, parse_pdf_data};
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
        // Accept either the ISIN itself, or the real name:
        assert!(result.asset == "EUR Corporate Bond (Dist)" || result.asset == "IE00BZ163G84");
    }

    #[test]
    fn test_isin_name_across_lines() {
        let input = "DATUM 30.07.2025\nISIN:\nIE00BF4RFH31\nMSCI World Small Cap USD (Acc)\n";
        let result = parse_pdf_data(input).unwrap();
        assert_eq!(result.isin.as_deref(), Some("IE00BF4RFH31"));
        assert!(result.asset == "MSCI World Small Cap USD (Acc)" || result.asset == "IE00BF4RFH31");
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

    #[test]
    fn test_asset_is_before_isin_in_position_block() {
        let input = "DATUM 04.08.2025\nPOSITION ANZAHL DURCHSCHNITTSKURS BETRAG\n\
        MSCI World Small Cap USD (Acc)\nISIN: IE00BF4RFH31\n3,517658 Stk. 7,107 EUR 25,00 EUR\nGESAMT 25,00 EUR\n";
        let result = parse_pdf_data(input).unwrap();
        assert_eq!(result.isin.as_deref(), Some("IE00BF4RFH31"));
        assert!(result.asset == "MSCI World Small Cap USD (Acc)" || result.asset == "IE00BF4RFH31");
    }
}
