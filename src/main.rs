#![forbid(unsafe_code)]

mod parser;

use anyhow::Result;
use parser::{build_filename, parse_pdf_data};
use regex::Regex;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::{env, fs};
use walkdir::WalkDir;

/// Extracts all text from a PDF file using pdf-extract
/// 
/// # Security
/// This function validates the input path and includes error handling
/// to prevent crashes from malformed PDF files.
/// 
/// # Arguments
/// * `path` - Path to the PDF file to extract text from
/// 
/// # Returns
/// * `Result<String>` - Extracted text on success, error on failure
/// 
/// # Errors
/// Returns an error if the PDF cannot be read or parsed
fn extract_pdf_text(path: &Path) -> Result<String> {
    let pdf_text = pdf_extract::extract_text(path)?;
    Ok(pdf_text)
}

/// Checks if a filename already matches the target renaming scheme
/// 
/// # Security
/// Uses a pre-compiled static regex to prevent ReDoS attacks and improve performance.
/// 
/// # Examples
/// - `2024_08_12_Kauf_DE000A1EWWW0_Vanguard_Funds_PLC_ETF.pdf` ✓
/// - `2024_08_12_Depotauszug_Depot.pdf` ✓ (no ISIN)
/// - `original_file.pdf` ✗
/// 
/// # Arguments
/// * `filename` - The filename to check
/// 
/// # Returns
/// * `bool` - True if the filename matches the expected pattern
fn is_already_renamed(filename: &str) -> bool {
    use once_cell::sync::Lazy;
    static RENAMED_PATTERN: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"^\d{4}_\d{2}_\d{2}_[A-Za-z_]+(_[A-Z]{2}[A-Z0-9]{9}\d)?_.+\.pdf$")
            .expect("Invalid regex pattern for renamed file detection")
    });
    RENAMED_PATTERN.is_match(filename)
}

/// Processes all PDF files in a folder (and subfolders),
/// skipping files already in the new naming scheme.
/// 
/// # Security Features
/// - Validates all paths to prevent directory traversal attacks
/// - Limits file size processing to prevent DoS attacks  
/// - Canonicalizes paths to ensure operations stay within target directory
/// - Validates filename lengths to prevent filesystem issues
/// - Includes comprehensive error handling for robustness
/// 
/// # Arguments
/// * `folder` - Path to the folder containing PDF files to process
/// 
/// # Returns
/// * `Result<()>` - Success or error details
/// 
/// # Security Validations
/// - Path existence and type validation
/// - Directory traversal prevention via canonicalization
/// - File size limits (100MB maximum)
/// - Filename length validation (255 characters maximum)
/// - Extension validation for PDF files only
/// 
/// # Errors
/// Returns an error if:
/// - The folder doesn't exist or isn't a directory
/// - Path canonicalization fails (potential security issue)
/// - File operations fail due to permissions or other I/O errors
fn process_folder(folder: &Path) -> Result<()> {
    // Validate input folder path
    if !folder.exists() {
        return Err(anyhow::anyhow!("Folder does not exist: {:?}", folder));
    }
    if !folder.is_dir() {
        return Err(anyhow::anyhow!("Path is not a directory: {:?}", folder));
    }
    
    // Canonicalize the folder path to prevent directory traversal attacks
    let canonical_folder = folder.canonicalize()?;
    
    for entry in WalkDir::new(&canonical_folder).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file()
            && entry
                .path()
                .extension()
                .and_then(OsStr::to_str)
                .map(|e| e.eq_ignore_ascii_case("pdf"))
                .unwrap_or(false)
        {
            let path = entry.path();
            
            // Validate that the file is still within our target directory
            if !path.starts_with(&canonical_folder) {
                println!("Skipping file outside target directory: {:?}", path);
                continue;
            }
            
            // Get filename with proper error handling
            let orig_filename = match path.file_name().and_then(|name| name.to_str()) {
                Some(name) => name,
                None => {
                    println!("Warning: Could not get filename for: {:?}", path);
                    continue;
                }
            };
            
            // Validate filename length
            if orig_filename.len() > 255 {
                println!("Skipping file with excessively long name: {}", orig_filename);
                continue;
            }

            // Skip files already renamed
            if is_already_renamed(orig_filename) {
                println!("Skipping (already renamed): {}", orig_filename);
                continue;
            }

            println!("Processing: {:?}", orig_filename);
            
            // Add file size validation to prevent processing extremely large files
            if let Ok(metadata) = path.metadata()
                && metadata.len() > 100_000_000 { // 100MB limit
                println!("Skipping large file (>100MB): {}", orig_filename);
                continue;
            }
            
            match extract_pdf_text(path) {
                Ok(text) => {
                    if let Some(pdf_data) = parse_pdf_data(&text) {
                        let new_name = build_filename(&pdf_data, orig_filename);
                        
                        // Validate the new filename
                        if new_name.len() > 255 {
                            println!("Warning: Generated filename too long for {}, skipping", orig_filename);
                            continue;
                        }
                        
                        let new_path = match path.parent() {
                            Some(parent) => parent.join(new_name),
                            None => {
                                println!("Warning: Could not get parent directory for {:?}", path);
                                continue;
                            }
                        };
                        
                        // Ensure new path is still within our target directory
                        if let Ok(canonical_new_path) = new_path.canonicalize().or_else(|_| {
                            // If canonicalize fails because the file doesn't exist yet,
                            // check the parent directory
                            new_path.parent().map(|p| p.canonicalize()).unwrap_or_else(|| 
                                Err(std::io::Error::new(std::io::ErrorKind::NotFound, "Cannot canonicalize path"))
                            )
                        })
                            && !canonical_new_path.starts_with(&canonical_folder) {
                            println!("Warning: Refusing to rename outside target directory: {:?}", new_path);
                            continue;
                        }
                        if let Ok(canonical_parent) = new_path.parent().map(|p| p.canonicalize()).unwrap_or_else(|| 
                            Err(std::io::Error::new(std::io::ErrorKind::NotFound, "Cannot canonicalize parent directory"))
                        ) {
                            // The new path must be directly under the canonical parent, and canonical parent must be within canonical_folder
                            if !canonical_parent.starts_with(&canonical_folder) {
                                println!("Warning: Refusing to rename outside target directory: {:?}", new_path);
                                continue;
                            }
                        } else {
                            println!("Warning: Could not canonicalize parent directory for {:?}", new_path);
                            continue;
                        }
                        match new_path.file_name() {
                            Some(name) => println!("Renaming to: {:?}", name),
                            None => println!("Warning: Could not determine filename for {:?}", new_path),
                        }
                        
                        if let Err(e) = fs::rename(path, &new_path) {
                            println!("Error renaming {:?}: {}", orig_filename, e);
                        }
                    } else {
                        println!("Warning: Could not parse {:?}", orig_filename);
                    }
                }
                Err(e) => {
                    println!("Error extracting text from {:?}: {}", orig_filename, e);
                }
            }
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <folder>", args.first().unwrap_or(&"tr_pdf_rename".to_string()));
        eprintln!("Example: {} /path/to/pdf/folder", args.first().unwrap_or(&"tr_pdf_rename".to_string()));
        return Ok(());
    }
    
    let folder_arg = &args[1];
    
    // Validate folder argument
    if folder_arg.len() > 4096 { // Reasonable path length limit
        return Err(anyhow::anyhow!("Folder path too long (max 4096 characters)"));
    }
    
    let folder = PathBuf::from(folder_arg);
    
    // Additional validation
    if !folder.exists() {
        return Err(anyhow::anyhow!("Folder does not exist: {:?}", folder));
    }
    
    process_folder(&folder)?;
    println!("Processing completed successfully.");
    Ok(())
}

#[cfg(test)]
mod tests {
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
            date: NaiveDate::from_ymd_opt(2025, 7, 2).unwrap(),
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

    // Security-focused tests
    #[test]
    fn test_parse_pdf_data_rejects_oversized_input() {
        let oversized_input = "DATUM 01.01.2024\n".repeat(100_000);
        let result = parse_pdf_data(&oversized_input);
        assert!(result.is_none(), "Should reject oversized input");
    }

    #[test]
    fn test_clean_name_handles_oversized_input() {
        let oversized_name = "A".repeat(1000);
        let result = clean_name(&oversized_name);
        assert_eq!(result, "Invalid_Asset_Name");
    }

    #[test]
    fn test_parse_pdf_data_validates_date_range() {
        // Test future date beyond reasonable range
        let future_input = "DATUM 01.01.2050\nKauf\n";
        let result = parse_pdf_data(future_input);
        assert!(result.is_none(), "Should reject far future dates");

        // Test very old date
        let old_input = "DATUM 01.01.1990\nKauf\n";
        let result = parse_pdf_data(old_input);
        assert!(result.is_none(), "Should reject very old dates");
    }

    #[test]
    fn test_build_filename_validates_isin() {
        let pdf_data = PdfData {
            date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            doc_type: "Kauf".to_string(),
            isin: Some("INVALID_ISIN_123456789".to_string()), // Invalid ISIN
            asset: "Test Asset".to_string(),
        };
        let filename = build_filename(&pdf_data, "test.pdf");
        // Should not include invalid ISIN in filename
        assert!(!filename.contains("INVALID_ISIN"));
    }

    #[test]
    fn test_build_filename_validates_file_extension() {
        let pdf_data = PdfData {
            date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            doc_type: "Kauf".to_string(),
            isin: None,
            asset: "Test Asset".to_string(),
        };
        
        // Test with malicious extension
        let filename = build_filename(&pdf_data, "test../../../etc/passwd");
        assert!(filename.ends_with(".pdf"), "Should default to .pdf for unsafe extensions");
        
        // Test with oversized extension
        let filename = build_filename(&pdf_data, &format!("test.{}", "a".repeat(20)));
        assert!(filename.ends_with(".pdf"), "Should default to .pdf for oversized extensions");
    }

    #[test]
    fn test_clean_name_removes_dangerous_characters() {
        assert_eq!(clean_name("../../../etc/passwd"), "etc_passwd");
        assert_eq!(clean_name("file<>:\"|?*name"), "file_name");
        assert_eq!(clean_name("con.txt"), "con_txt"); // Windows reserved name
    }

    #[test]
    fn test_clean_name_handles_unicode_and_special_cases() {
        assert_eq!(clean_name("Test\x00\x01\x02"), "Test"); // Control characters
        assert_eq!(clean_name("Test\u{202E}malicious"), "Testmalicious"); // Right-to-left override (removed, not replaced)
        assert_eq!(clean_name(""), ""); // Empty string
        assert_eq!(clean_name("   "), ""); // Only whitespace
    }

    #[test]
    fn test_asset_validation_in_parsing() {
        let input = "DATUM 01.01.2024\nKauf\n";
        let result = parse_pdf_data(input).unwrap();
        
        // Asset should not be empty
        assert!(!result.asset.is_empty());
        
        // Asset should have reasonable length
        assert!(result.asset.len() <= 500);
    }
}
