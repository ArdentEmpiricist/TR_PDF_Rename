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

    #[test]
    fn test_already_renamed_with_isin() {
        assert!(is_already_renamed(
            "2024_08_12_Kauf_IE00BZ163G84_My_ETF_Name.pdf"
        ));
        assert!(is_already_renamed(
            "2023_11_30_Kauf_DE000A1EWWW0_Vanguard_ETF.pdf"
        ));
    }

    #[test]
    fn test_already_renamed_without_isin() {
        assert!(is_already_renamed("2022_05_11_Depotauszug_Depot.pdf"));
        assert!(is_already_renamed(
            "2021_12_01_Steuerliche_Optimierung_Steuer.pdf"
        ));
    }

    #[test]
    fn test_not_renamed_yet() {
        assert!(!is_already_renamed("Abrechnung 04.03.2024.pdf"));
        assert!(!is_already_renamed("Trade Republic - Kauf.pdf"));
        assert!(!is_already_renamed("dividende-test.pdf"));
        assert!(!is_already_renamed("12345.pdf"));
    }
}
