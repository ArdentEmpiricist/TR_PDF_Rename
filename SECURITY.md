# Security Policy

## Supported Versions

Currently supported versions for security updates:

| Version | Supported          |
| ------- | ------------------ |
| 0.2.6+  | :white_check_mark: |
| < 0.2.6 | :x:                |

## Security Features

This project implements several security measures to ensure safe operation:

### Input Validation & Sanitization

- **File Size Limits**: Maximum 100MB per PDF to prevent DoS attacks
- **Path Validation**: All paths are canonicalized to prevent directory traversal
- **Character Sanitization**: Dangerous characters are removed from filenames
- **Length Limits**: Input validation for all text fields and paths
- **Date Validation**: Reasonable date range validation (2000-2030)
- **ISIN Validation**: Proper checksum verification for ISIN codes

### File System Security

- **Directory Traversal Protection**: Prevents `../` attacks
- **Path Canonicalization**: Ensures operations stay within target directory
- **Extension Validation**: Only processes legitimate PDF files
- **Permission Checks**: Validates file accessibility before processing

### Memory Safety

- **No Unsafe Code**: Uses `#![forbid(unsafe_code)]` directive
- **Static Analysis**: All regex patterns pre-compiled to prevent ReDoS
- **Error Handling**: Comprehensive error handling prevents panics

### Character Set Security

- **Unicode Attack Prevention**: Removes directional override characters
- **Control Character Filtering**: Strips potentially dangerous control chars
- **Filesystem Safety**: Replaces characters that could cause filesystem issues

## Security Testing

The project includes comprehensive security tests covering:

- Oversized input rejection
- Path traversal attempt prevention  
- Malicious filename character handling
- Unicode security exploit prevention
- File size limit enforcement
- Date range validation
- ISIN validation integrity

Run security tests with:
```bash
cargo test
```

## Reporting a Vulnerability

We take security seriously. If you discover a security vulnerability, please:

### For Security Issues:

1. **DO NOT** open a public issue
2. Email the maintainers directly at [security contact needed]
3. Include:
   - Description of the vulnerability
   - Steps to reproduce
   - Potential impact assessment
   - Suggested fix (if available)

### Response Timeline:

- **Acknowledgment**: Within 48 hours
- **Initial Assessment**: Within 7 days  
- **Fix Development**: Depends on severity
- **Patch Release**: As soon as safely possible

### Severity Levels:

- **Critical**: Remote code execution, data corruption
- **High**: Local privilege escalation, significant data exposure
- **Medium**: Local denial of service, minor data exposure
- **Low**: Information disclosure, edge case vulnerabilities

## Security Best Practices for Users

When using this tool:

1. **Sandbox Environment**: Run in isolated environment for untrusted PDFs
2. **File Permissions**: Ensure appropriate read/write permissions
3. **Backup Data**: Always backup important files before processing
4. **Regular Updates**: Keep dependencies updated with `cargo update`
5. **Input Validation**: Verify PDF sources are trusted
6. **Monitor Output**: Review renamed files for accuracy

## Dependencies Security

This project uses only well-maintained, security-audited dependencies:

- `chrono`: Date/time handling - actively maintained
- `regex`: Pattern matching - security-hardened with static compilation
- `anyhow`: Error handling - lightweight and safe
- `isin`: ISIN validation - includes proper checksum verification
- `pdf-extract`: PDF text extraction - sandboxed operation
- `walkdir`: Directory traversal - safe directory walking
- `once_cell`: Static initialization - memory-safe lazy statics

Run security audit with:
```bash
cargo audit
```

## Secure Development Practices

This project follows secure development practices:

- **Static Analysis**: Regular clippy and rustfmt checks
- **Dependency Scanning**: Automated dependency vulnerability scanning
- **Code Review**: All changes reviewed for security implications
- **Testing**: Comprehensive test suite including security scenarios
- **Documentation**: Security considerations documented throughout

## Compliance & Standards

- **Memory Safety**: Rust's memory safety guarantees prevent common vulnerabilities
- **Input Validation**: Following OWASP guidelines for input validation
- **Error Handling**: Defensive programming practices implemented
- **Principle of Least Privilege**: Minimal required permissions
- **Defense in Depth**: Multiple layers of security validation

## Changelog

### Version 0.2.6
- Added comprehensive input validation
- Implemented path canonicalization security
- Enhanced character sanitization
- Added file size limits
- Implemented static regex compilation
- Added security-focused test suite
- Enhanced error handling throughout

For questions about this security policy, please open an issue or contact the maintainers.