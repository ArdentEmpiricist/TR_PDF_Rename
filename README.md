[![Rust](https://github.com/ArdentEmpiricist/TR_PDF_Rename/actions/workflows/rust.yml/badge.svg)](https://github.com/ArdentEmpiricist/TR_PDF_Rename/actions/workflows/rust.yml)
![Crates.io License](https://img.shields.io/crates/l/tr_pdf_rename)

Solves a niche problem, but may be useful to some.

The neo-broker Trade Republic does not name security transaction statements in a reasonable way. So TR_PDF_Rename helps to archive the documents and renames all PDF files with the pattern date(yyyy_mm_dd)_transactiontype_stockname. Example: ```2024_01_01_WERTPAPIERABRECHNUNG_MSCI World USD (Dist)```

### Warning: 
Should only be used on PDF files from Trade Republic or directories only containing these files! Will panic if used with non Trade Republic PDF files.

### how to:
install ```cargo add TR_PDF_rename``` or download from https://github.com/ArdentEmpiricist/TR_PDF_Rename/releases

use ```TR_PDF_rename [path]```