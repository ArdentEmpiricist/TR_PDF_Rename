[![Rust](https://github.com/ArdentEmpiricist/TR_PDF_Rename/actions/workflows/rust.yml/badge.svg)](https://github.com/ArdentEmpiricist/TR_PDF_Rename/actions/workflows/rust.yml)
[![Clippy check](https://github.com/ArdentEmpiricist/TR_PDF_Rename/actions/workflows/clippy.yml/badge.svg)](https://github.com/ArdentEmpiricist/TR_PDF_Rename/actions/workflows/clippy.yml)
![Crates.io License](https://img.shields.io/crates/l/tr_pdf_rename)

# Trade Republic PDF Rename

A secure and robust tool that renames Trade Republic PDF documents to a structured, machine-readable format:

```
yyyy_mm_dd_[TYPE]_[ISIN]_[ASSET].pdf
```

## Features

- **Automated PDF Processing**: Recursively processes all PDF files in a directory
- **Intelligent Document Type Recognition**: Supports various Trade Republic document types
- **ISIN Validation**: Validates and includes ISIN codes when available
- **Security-Hardened**: Input validation, path sanitization, and protection against common attack vectors
- **Safe File Operations**: Prevents directory traversal attacks and validates all file paths
- **Comprehensive Error Handling**: Graceful handling of malformed PDFs and edge cases

## Supported Document Types

- `Kauf` - Purchase transactions
- `Kauf_Sparplan` - Savings plan purchases  
- `Kauf_Saveback` - Saveback purchases
- `Verkauf` - Sale transactions
- `Dividende` - Dividend payments
- `Zinsen` - Interest payments
- `Zinszahlung` - Interest payouts
- `Kapitalmaßnahme` - Corporate actions
- `Depottransfer` - Depot transfers
- `Depotauszug` - Account statements
- `Steuerliche_Optimierung` - Tax optimization

## Installation & Usage

### Build from Source

```bash
git clone https://github.com/ArdentEmpiricist/TR_PDF_Rename.git
cd TR_PDF_Rename
cargo build --release
```

### Run

```bash
./target/release/tr_pdf_rename <path_to_folder>
```

### Example

```bash
./target/release/tr_pdf_rename ~/Documents/TradeRepublic/
```

## Security Features

This tool implements several security measures to ensure safe operation:

### Input Validation

- **File Size Limits**: Rejects files larger than 100MB to prevent DoS attacks
- **Path Validation**: Ensures all operations stay within the target directory
- **Character Sanitization**: Removes dangerous characters from filenames
- **Length Limits**: Validates filename and path lengths

### Safe File Operations

- **Directory Traversal Protection**: Prevents `../` attacks through path canonicalization
- **Extension Validation**: Validates file extensions to prevent malicious files
- **ISIN Validation**: Proper validation of ISIN codes using checksum verification

### Error Handling

- **Graceful Degradation**: Continues processing other files if one fails
- **Input Sanitization**: Removes control characters and Unicode exploits
- **Memory Safety**: Uses `#![forbid(unsafe_code)]` for guaranteed memory safety

## Testing

Run the comprehensive test suite including security tests:

```bash
cargo test
```

Run with verbose output:

```bash
cargo test -- --nocapture
```

## Contributing

1. Fork the repository
2. Create a feature branch
3. Implement your changes with tests
4. Ensure all tests pass
5. Submit a pull request

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
