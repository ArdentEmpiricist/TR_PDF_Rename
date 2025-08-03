[![Rust](https://github.com/ArdentEmpiricist/TR_PDF_Rename/actions/workflows/rust.yml/badge.svg)](https://github.com/ArdentEmpiricist/TR_PDF_Rename/actions/workflows/rust.yml)
[![Clippy check](https://github.com/ArdentEmpiricist/TR_PDF_Rename/actions/workflows/clippy.yml/badge.svg)](https://github.com/ArdentEmpiricist/TR_PDF_Rename/actions/workflows/clippy.yml)
![Crates.io License](https://img.shields.io/crates/l/tr_pdf_rename)

# Trade Republic PDF Rename

This tool renames Trade Republic PDF documents to a structured, machine-readable format:

    yyyy_mm_dd_[TYPE]_[ASSET].pdf

Supported Types: Kauf, Kauf_Sparplan, Kauf_Saveback, Verkauf, Dividende, Zinsen, Zinszahlung, Kapitalmaßnahme, Depottransfer, Depotauszug, Steuerliche_Optimierung

## Usage

1. Build: `cargo build --release`
2. Run: `./target/release/tr_pdf_rename <path_to_folder>`
3. Test: `cargo test`
